use super::alu::{self, AluResult8, IncDecResult8};
use super::interrupts;
use super::stack::{pop16, push16};
use super::{Cond, Cpu, Decoded, Instruction, Operand8, Reg8, Reg16, decode};
use crate::bus::Bus;

/// Why a CPU step could not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    Decode {
        pc: u16,
        /// Up to 8 bytes starting at `pc` (for diagnostics).
        bytes: [u8; 8],
    },
    Unimplemented {
        pc: u16,
        instruction: Instruction,
    },
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode { pc, bytes } => {
                write!(f, "failed to decode opcode ${:02X} at ${pc:04X}", bytes[0])?;
                write!(f, " (")?;
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{b:02X}")?;
                }
                write!(f, ")")
            }
            Self::Unimplemented { pc, instruction } => write!(
                f,
                "unimplemented {} at ${pc:04X}",
                instruction.mnemonic(*pc)
            ),
        }
    }
}

impl std::error::Error for StepError {}

/// Fetch/decode one instruction at `cpu.pc`, advance PC by its length, then execute.
///
/// Control-flow instructions may override PC after the sequential advance.
///
/// Interrupt poll/wake/service runs first (see [`interrupts::poll`]). While halted
/// with nothing pending, no opcode is fetched (`Bus::tick` still advances timer/PPU).
pub fn step(cpu: &mut Cpu, bus: &mut Bus) -> Result<Decoded, StepError> {
    // AntonioND TCAGBD §4.9: interrupt dispatch is 5 M-cycles (20 T), plus one
    // extra M-cycle (4 T) when the CPU was halted.
    let from_halt = cpu.halted;
    if interrupts::poll(cpu, bus) {
        // Pan Docs: HDMA also halts; resume DMA when the CPU wakes.
        bus.set_cpu_halted(cpu.halted);
        bus.tick(if from_halt { 24 } else { 20 });
        return Ok(Decoded {
            instruction: Instruction::Nop,
            len: 0,
        });
    }

    bus.set_cpu_halted(cpu.halted);
    if cpu.halted {
        // Clock keeps running while halted.
        bus.tick(4);
        return Ok(Decoded {
            instruction: Instruction::Halt,
            len: 0,
        });
    }

    let pc = cpu.pc;
    let preview = peek_bytes(bus, pc);
    let decoded = decode(&[preview[0], preview[1], preview[2]])
        .ok_or(StepError::Decode { pc, bytes: preview })?;

    cpu.pc = cpu.pc.wrapping_add(u16::from(decoded.len));
    // M-cycle phase: advance the bus before memory operands so MMIO (STAT/LY/…)
    // samples mid-instruction, not after the whole instruction budget.
    let t_cycles = approx_t_cycles(decoded.instruction);
    let pre = mem_operand_pre_t(decoded.instruction);
    // Taken control-flow extras (Pan Docs / pastraiser).
    let taken_extra = match decoded.instruction {
        Instruction::Jr { cond, .. } if cond_met(cpu, cond) => 4,
        Instruction::Jp { cond, .. } if cond_met(cpu, cond) => 4,
        Instruction::Call { cond, .. } if cond_met(cpu, cond) => 12,
        Instruction::Ret { cond: Some(c) } if cond_met(cpu, Some(c)) => 12,
        _ => 0,
    };
    if pre > 0 {
        bus.tick(pre);
    }
    if let Err(instruction) = execute(decoded.instruction, cpu, bus) {
        return Err(StepError::Unimplemented { pc, instruction });
    }
    interrupts::apply_delayed_ei(cpu);
    let stop_switched = matches!(decoded.instruction, Instruction::Stop) && bus.on_stop();
    if !stop_switched {
        bus.tick(t_cycles.saturating_sub(pre).saturating_add(taken_extra));
    }
    Ok(decoded)
}

/// Eight bytes at `addr` for fault / ridge dumps.
pub fn peek_bytes(bus: &Bus, addr: u16) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (i, b) in out.iter_mut().enumerate() {
        *b = bus.read8(addr.wrapping_add(i as u16));
    }
    out
}

/// T-cycles to advance before executing an instruction that touches memory in a
/// later M-cycle (so PPU/timer MMIO sees the mid-instruction phase).
fn mem_operand_pre_t(ins: Instruction) -> u32 {
    use Instruction::*;
    match ins {
        // 2 M-cycles: opcode fetch, then (HL) access.
        Ld8 {
            dst: Operand8::MemHl,
            src: Operand8::Imm(_),
        } => 8,
        Ld8 {
            src: Operand8::MemHl,
            ..
        }
        | Ld8 {
            dst: Operand8::MemHl,
            ..
        }
        | Inc8(Operand8::MemHl)
        | Dec8(Operand8::MemHl)
        | Add(Operand8::MemHl)
        | Adc(Operand8::MemHl)
        | Sub(Operand8::MemHl)
        | Sbc(Operand8::MemHl)
        | And(Operand8::MemHl)
        | Xor(Operand8::MemHl)
        | Or(Operand8::MemHl)
        | Cp(Operand8::MemHl)
        | LdiMemHlA
        | LdiAMemHl
        | LddMemHlA
        | LddAMemHl
        | LdMemBcA
        | LdAMemBc
        | LdMemDeA
        | LdAMemDe => 4,
        // 3 M-cycles: opcode, immediate, then $FF00+n access.
        LdhAbsA(_) | LdhAAbs(_) => 8,
        LdIoCA | LdAIoC => 4,
        _ => 0,
    }
}

/// Staged cycle estimate in T-cycles (4 T = 1 M). Good enough for timer progress;
/// not a substitute for a full timing table.
fn approx_t_cycles(ins: Instruction) -> u32 {
    use Instruction::*;
    match ins {
        Nop | Di | Ei | Scf | Ccf | Cpl | JpHl | Halt | Stop | Rlca | Rrca | Rla | Rra | Daa => 4,
        Inc16(_) | Dec16(_) | AddHl(_) | LdSpHl => 8,
        Jr { .. } => 8, // not-taken; +4 T when taken
        LdHlSpE(_) => 12,
        Jp { .. } => 12,   // uncond / taken +4 → 16; cc not-taken 12
        Call { .. } => 12, // uncond / taken +12 → 24; cc not-taken 12
        Ret { cond: None } | Reti => 16,
        Ret { .. } => 8, // cc not-taken; +12 T when taken → 20
        Rst(_) => 16,
        Push(_) | AddSpE(_) => 16,
        Pop(_) => 12,
        Ld16Imm { .. } => 12,
        LdAbsSp(_) => 20,
        LdAbsA(_) | LdAAbs(_) => 16,
        LdhAbsA(_) | LdhAAbs(_) | LdIoCA | LdAIoC => 12,
        LdiMemHlA | LdiAMemHl | LddMemHlA | LddAMemHl | LdMemBcA | LdAMemBc | LdMemDeA
        | LdAMemDe => 8,
        Inc8(Operand8::MemHl) | Dec8(Operand8::MemHl) => 12,
        Inc8(_) | Dec8(_) => 4,
        Ld8 {
            dst: Operand8::MemHl,
            src: Operand8::Imm(_),
        } => 12,
        Ld8 {
            src: Operand8::Imm(_),
            ..
        } => 8,
        Ld8 {
            dst: Operand8::MemHl,
            ..
        }
        | Ld8 {
            src: Operand8::MemHl,
            ..
        } => 8,
        Ld8 { .. } => 4,
        Add(Operand8::Imm(_))
        | Adc(Operand8::Imm(_))
        | Sub(Operand8::Imm(_))
        | Sbc(Operand8::Imm(_))
        | And(Operand8::Imm(_))
        | Xor(Operand8::Imm(_))
        | Or(Operand8::Imm(_))
        | Cp(Operand8::Imm(_)) => 8,
        Add(Operand8::MemHl) | Adc(Operand8::MemHl) | Sub(Operand8::MemHl)
        | Sbc(Operand8::MemHl) | And(Operand8::MemHl) | Xor(Operand8::MemHl)
        | Or(Operand8::MemHl) | Cp(Operand8::MemHl) => 8,
        Add(_) | Adc(_) | Sub(_) | Sbc(_) | And(_) | Xor(_) | Or(_) | Cp(_) => 4,
        Bit {
            op: Operand8::MemHl,
            ..
        } => 12,
        Rlc(Operand8::MemHl)
        | Rrc(Operand8::MemHl)
        | Rl(Operand8::MemHl)
        | Rr(Operand8::MemHl)
        | Sla(Operand8::MemHl)
        | Sra(Operand8::MemHl)
        | Srl(Operand8::MemHl)
        | Swap(Operand8::MemHl)
        | Res {
            op: Operand8::MemHl,
            ..
        }
        | Set {
            op: Operand8::MemHl,
            ..
        } => 16,
        Rlc(_)
        | Rrc(_)
        | Rl(_)
        | Rr(_)
        | Sla(_)
        | Sra(_)
        | Srl(_)
        | Swap(_)
        | Bit { .. }
        | Res { .. }
        | Set { .. } => 8,
    }
}

fn execute(ins: Instruction, cpu: &mut Cpu, bus: &mut Bus) -> Result<(), Instruction> {
    match ins {
        Instruction::Nop => Ok(()),

        Instruction::Halt => {
            cpu.halted = true;
            bus.set_cpu_halted(true);
            Ok(())
        }

        Instruction::Stop => {
            // DMG / unarmed CGB: 2-byte NOP (joypad-wait STOP is not modeled).
            // Armed CGB speed switch runs in `step` via [`Bus::on_stop`].
            Ok(())
        }

        Instruction::Jp { cond, target } => {
            if cond_met(cpu, cond) {
                cpu.pc = target;
            }
            Ok(())
        }

        Instruction::JpHl => {
            cpu.pc = cpu.hl();
            Ok(())
        }

        Instruction::Jr { cond, offset } => {
            if cond_met(cpu, cond) {
                // PC already points past the JR; apply signed displacement.
                cpu.pc = cpu.pc.wrapping_add(offset as i16 as u16);
            }
            Ok(())
        }

        Instruction::Call { cond, target } => {
            if cond_met(cpu, cond) {
                push16(cpu, bus, cpu.pc);
                cpu.pc = target;
            }
            Ok(())
        }

        Instruction::Rst(vec) => {
            push16(cpu, bus, cpu.pc);
            cpu.pc = u16::from(vec);
            Ok(())
        }

        Instruction::Ret { cond } => {
            if cond_met(cpu, cond) {
                cpu.pc = pop16(cpu, bus);
            }
            Ok(())
        }

        Instruction::Reti => {
            cpu.pc = pop16(cpu, bus);
            cpu.ime = true;
            cpu.ime_enable_pending = false;
            cpu.ime_enable_armed = false;
            Ok(())
        }

        Instruction::Push(rr) => {
            push16(cpu, bus, cpu.read_r16(rr));
            Ok(())
        }

        Instruction::Pop(rr) => {
            let value = pop16(cpu, bus);
            cpu.write_r16(rr, value);
            Ok(())
        }

        Instruction::Ld16Imm { dst, value } => {
            cpu.write_r16(dst, value);
            Ok(())
        }

        Instruction::LdAbsSp(addr) => {
            bus.write16(addr, cpu.sp);
            Ok(())
        }

        Instruction::Inc16(rr) => {
            let v = cpu.read_r16(rr).wrapping_add(1);
            cpu.write_r16(rr, v);
            Ok(())
        }

        Instruction::Dec16(rr) => {
            let v = cpu.read_r16(rr).wrapping_sub(1);
            cpu.write_r16(rr, v);
            Ok(())
        }

        Instruction::AddHl(rr) => {
            let r = alu::add_hl(cpu.read_r16(Reg16::Hl), cpu.read_r16(rr));
            cpu.write_r16(Reg16::Hl, r.value);
            cpu.set_flag_n(false);
            cpu.set_flag_h(r.h);
            cpu.set_flag_c(r.c);
            Ok(())
        }

        Instruction::LdHlSpE(e) => {
            let r = alu::add_sp_e8(cpu.sp, e);
            cpu.write_r16(Reg16::Hl, r.value);
            cpu.set_flag_z(false);
            cpu.set_flag_n(false);
            cpu.set_flag_h(r.h);
            cpu.set_flag_c(r.c);
            Ok(())
        }

        Instruction::LdSpHl => {
            cpu.sp = cpu.read_r16(Reg16::Hl);
            Ok(())
        }

        Instruction::AddSpE(e) => {
            let r = alu::add_sp_e8(cpu.sp, e);
            cpu.sp = r.value;
            cpu.set_flag_z(false);
            cpu.set_flag_n(false);
            cpu.set_flag_h(r.h);
            cpu.set_flag_c(r.c);
            Ok(())
        }

        Instruction::Ld8 { dst, src } => {
            let value = read_operand(cpu, bus, src)?;
            write_operand(cpu, bus, dst, value)?;
            Ok(())
        }

        Instruction::LdAbsA(addr) => {
            bus.write8(addr, cpu.a);
            Ok(())
        }

        Instruction::LdAAbs(addr) => {
            cpu.a = bus.read8(addr);
            Ok(())
        }

        Instruction::LdMemBcA => {
            bus.write8(cpu.read_r16(Reg16::Bc), cpu.a);
            Ok(())
        }

        Instruction::LdAMemBc => {
            cpu.a = bus.read8(cpu.read_r16(Reg16::Bc));
            Ok(())
        }

        Instruction::LdMemDeA => {
            bus.write8(cpu.read_r16(Reg16::De), cpu.a);
            Ok(())
        }

        Instruction::LdAMemDe => {
            cpu.a = bus.read8(cpu.read_r16(Reg16::De));
            Ok(())
        }

        Instruction::LdhAbsA(offset) => {
            bus.write8(0xFF00 | u16::from(offset), cpu.a);
            Ok(())
        }

        Instruction::LdhAAbs(offset) => {
            cpu.a = bus.read8(0xFF00 | u16::from(offset));
            Ok(())
        }

        Instruction::LdIoCA => {
            bus.write8(0xFF00 | u16::from(cpu.c), cpu.a);
            Ok(())
        }

        Instruction::LdAIoC => {
            cpu.a = bus.read8(0xFF00 | u16::from(cpu.c));
            Ok(())
        }

        Instruction::LdiMemHlA => {
            let hl = cpu.read_r16(Reg16::Hl);
            bus.write8(hl, cpu.a);
            cpu.write_r16(Reg16::Hl, hl.wrapping_add(1));
            Ok(())
        }

        Instruction::LdiAMemHl => {
            let hl = cpu.read_r16(Reg16::Hl);
            cpu.a = bus.read8(hl);
            cpu.write_r16(Reg16::Hl, hl.wrapping_add(1));
            Ok(())
        }

        Instruction::LddMemHlA => {
            let hl = cpu.read_r16(Reg16::Hl);
            bus.write8(hl, cpu.a);
            cpu.write_r16(Reg16::Hl, hl.wrapping_sub(1));
            Ok(())
        }

        Instruction::LddAMemHl => {
            let hl = cpu.read_r16(Reg16::Hl);
            cpu.a = bus.read8(hl);
            cpu.write_r16(Reg16::Hl, hl.wrapping_sub(1));
            Ok(())
        }

        Instruction::Inc8(op) => {
            let v = read_operand(cpu, bus, op)?;
            let r = alu::inc8(v);
            write_operand(cpu, bus, op, r.value)?;
            apply_inc_dec_flags(cpu, r);
            Ok(())
        }

        Instruction::Dec8(op) => {
            let v = read_operand(cpu, bus, op)?;
            let r = alu::dec8(v);
            write_operand(cpu, bus, op, r.value)?;
            apply_inc_dec_flags(cpu, r);
            Ok(())
        }

        Instruction::Add(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::add8(cpu.a, b);
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Adc(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::adc8(cpu.a, b, cpu.flag_c());
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Sub(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::sub8(cpu.a, b);
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Sbc(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::sbc8(cpu.a, b, cpu.flag_c());
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Cp(op) => {
            let b = read_operand(cpu, bus, op)?;
            apply_alu_flags(cpu, alu::cp8(cpu.a, b));
            Ok(())
        }

        Instruction::Xor(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::xor8(cpu.a, b);
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::And(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::and8(cpu.a, b);
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Or(op) => {
            let b = read_operand(cpu, bus, op)?;
            let r = alu::or8(cpu.a, b);
            cpu.a = r.value;
            apply_alu_flags(cpu, r);
            Ok(())
        }

        Instruction::Di => {
            cpu.ime = false;
            cpu.ime_enable_pending = false;
            cpu.ime_enable_armed = false;
            Ok(())
        }

        Instruction::Ei => {
            // IME becomes true at the start of the next instruction (see interrupts::poll).
            cpu.ime_enable_pending = true;
            Ok(())
        }

        Instruction::Scf => {
            cpu.set_flag_n(false);
            cpu.set_flag_h(false);
            cpu.set_flag_c(true);
            Ok(())
        }

        Instruction::Ccf => {
            cpu.set_flag_n(false);
            cpu.set_flag_h(false);
            cpu.set_flag_c(!cpu.flag_c());
            Ok(())
        }

        Instruction::Daa => {
            let r = alu::daa(cpu.a, cpu.flag_n(), cpu.flag_h(), cpu.flag_c());
            cpu.a = r.value;
            cpu.set_flag_z(r.z);
            cpu.set_flag_n(r.n);
            cpu.set_flag_h(r.h);
            cpu.set_flag_c(r.c);
            Ok(())
        }

        Instruction::Cpl => {
            cpu.a = !cpu.a;
            cpu.set_flag_n(true);
            cpu.set_flag_h(true);
            Ok(())
        }

        Instruction::Swap(op) => {
            let v = read_operand(cpu, bus, op)?;
            let swapped = v.rotate_right(4);
            write_operand(cpu, bus, op, swapped)?;
            cpu.set_flag_z(swapped == 0);
            cpu.set_flag_n(false);
            cpu.set_flag_h(false);
            cpu.set_flag_c(false);
            Ok(())
        }

        Instruction::Rlca => {
            let r = alu::rlc(cpu.a);
            cpu.a = r.value;
            apply_acc_rotate_flags(cpu, r.c);
            Ok(())
        }
        Instruction::Rrca => {
            let r = alu::rrc(cpu.a);
            cpu.a = r.value;
            apply_acc_rotate_flags(cpu, r.c);
            Ok(())
        }
        Instruction::Rla => {
            let r = alu::rl(cpu.a, cpu.flag_c());
            cpu.a = r.value;
            apply_acc_rotate_flags(cpu, r.c);
            Ok(())
        }
        Instruction::Rra => {
            let r = alu::rr(cpu.a, cpu.flag_c());
            cpu.a = r.value;
            apply_acc_rotate_flags(cpu, r.c);
            Ok(())
        }

        Instruction::Rlc(op) => rotate_op(cpu, bus, op, |v, _| alu::rlc(v)),
        Instruction::Rrc(op) => rotate_op(cpu, bus, op, |v, _| alu::rrc(v)),
        Instruction::Rl(op) => rotate_op(cpu, bus, op, alu::rl),
        Instruction::Rr(op) => rotate_op(cpu, bus, op, alu::rr),
        Instruction::Sla(op) => rotate_op(cpu, bus, op, |v, _| alu::sla(v)),
        Instruction::Sra(op) => rotate_op(cpu, bus, op, |v, _| alu::sra(v)),
        Instruction::Srl(op) => rotate_op(cpu, bus, op, |v, _| alu::srl(v)),

        Instruction::Bit { bit, op } => {
            let v = read_operand(cpu, bus, op)?;
            let f = alu::bit(v, bit);
            cpu.set_flag_z(f.z);
            cpu.set_flag_n(f.n);
            cpu.set_flag_h(f.h);
            Ok(())
        }

        Instruction::Res { bit, op } => {
            let v = read_operand(cpu, bus, op)? & !(1 << bit);
            write_operand(cpu, bus, op, v)
        }

        Instruction::Set { bit, op } => {
            let v = read_operand(cpu, bus, op)? | (1 << bit);
            write_operand(cpu, bus, op, v)
        }
    }
}

fn cond_met(cpu: &Cpu, cond: Option<Cond>) -> bool {
    match cond {
        None => true,
        Some(Cond::Z) => cpu.flag_z(),
        Some(Cond::Nz) => !cpu.flag_z(),
        Some(Cond::C) => cpu.flag_c(),
        Some(Cond::Nc) => !cpu.flag_c(),
    }
}

fn apply_alu_flags(cpu: &mut Cpu, r: AluResult8) {
    cpu.set_flag_z(r.z);
    cpu.set_flag_n(r.n);
    cpu.set_flag_h(r.h);
    cpu.set_flag_c(r.c);
}

/// Non-CB accumulator rotates: Z always 0 (unlike CB RLC/RL/…).
fn apply_acc_rotate_flags(cpu: &mut Cpu, carry: bool) {
    cpu.set_flag_z(false);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(carry);
}

fn apply_inc_dec_flags(cpu: &mut Cpu, r: IncDecResult8) {
    cpu.set_flag_z(r.z);
    cpu.set_flag_n(r.n);
    cpu.set_flag_h(r.h);
}

fn rotate_op(
    cpu: &mut Cpu,
    bus: &mut Bus,
    op: Operand8,
    f: impl FnOnce(u8, bool) -> AluResult8,
) -> Result<(), Instruction> {
    let v = read_operand(cpu, bus, op)?;
    let r = f(v, cpu.flag_c());
    write_operand(cpu, bus, op, r.value)?;
    apply_alu_flags(cpu, r);
    Ok(())
}

fn read_operand(cpu: &Cpu, bus: &Bus, op: Operand8) -> Result<u8, Instruction> {
    match op {
        Operand8::Reg(r) => Ok(cpu.read_r8(r)),
        Operand8::Imm(n) => Ok(n),
        Operand8::MemHl => Ok(bus.read8(cpu.read_r16(Reg16::Hl))),
    }
}

fn write_operand(cpu: &mut Cpu, bus: &mut Bus, op: Operand8, value: u8) -> Result<(), Instruction> {
    match op {
        Operand8::Reg(r) => {
            cpu.write_r8(r, value);
            Ok(())
        }
        Operand8::Imm(_) => Err(Instruction::Ld8 {
            dst: op,
            src: Operand8::Reg(Reg8::A),
        }),
        Operand8::MemHl => {
            bus.write8(cpu.read_r16(Reg16::Hl), value);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;

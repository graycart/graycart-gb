use super::interrupts;
use super::{Cpu, Decoded, Instruction, Operand8, decode};
use crate::bus::Bus;

mod ops;

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
        Instruction::Jr { cond, .. } if ops::cond_met(cpu, cond) => 4,
        Instruction::Jp { cond, .. } if ops::cond_met(cpu, cond) => 4,
        Instruction::Call { cond, .. } if ops::cond_met(cpu, cond) => 12,
        Instruction::Ret { cond: Some(c) } if ops::cond_met(cpu, Some(c)) => 12,
        _ => 0,
    };
    if pre > 0 {
        bus.tick(pre);
    }
    if let Err(instruction) = ops::execute(decoded.instruction, cpu, bus) {
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

#[cfg(test)]
mod tests;

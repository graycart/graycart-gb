//! Decoded instruction handlers and operand helpers.
//!
//! [`super::step`] owns fetch/decode/timing; this module applies one instruction.

use super::super::alu::{self, AluResult8, IncDecResult8};
use super::super::stack::{pop16, push16};
use super::super::{Cond, Cpu, Instruction, Operand8, Reg8, Reg16};
use crate::bus::Bus;

pub(super) fn execute(ins: Instruction, cpu: &mut Cpu, bus: &mut Bus) -> Result<(), Instruction> {
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

pub(super) fn cond_met(cpu: &Cpu, cond: Option<Cond>) -> bool {
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

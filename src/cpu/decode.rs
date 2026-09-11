use super::{Cond, Instruction, Operand8, Reg8, Reg16};

/// One successfully decoded instruction and its length in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoded {
    pub instruction: Instruction,
    pub len: u8,
}

/// Decode one instruction from `bytes`.
///
/// Returns `None` for unknown opcodes or when the slice is truncated mid-instruction.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    let opcode = *bytes.first()?;
    match opcode {
        // --- 1-byte ---
        0x00 => ok(Instruction::Nop, 1),
        0x07 => ok(Instruction::Rlca, 1),
        0x0F => ok(Instruction::Rrca, 1),
        0x17 => ok(Instruction::Rla, 1),
        0x1F => ok(Instruction::Rra, 1),
        0x2F => ok(Instruction::Cpl, 1),
        0x37 => ok(Instruction::Scf, 1),
        0x3F => ok(Instruction::Ccf, 1),
        0x27 => ok(Instruction::Daa, 1),
        0x08 => imm16(bytes, Instruction::LdAbsSp),
        0x10 => {
            // STOP is 2 bytes; second is usually $00 (ignored for now).
            if bytes.len() < 2 {
                return None;
            }
            ok(Instruction::Stop, 2)
        }
        // HALT sits in the LD r,r block between $75 and $77
        0x76 => ok(Instruction::Halt, 1),

        // ALU A,r / A,(HL)
        0x80..=0x87 => alu_r(opcode, Instruction::Add),
        0x88..=0x8F => alu_r(opcode, Instruction::Adc),
        0x90..=0x97 => alu_r(opcode, Instruction::Sub),
        0x98..=0x9F => alu_r(opcode, Instruction::Sbc),
        0xA0..=0xA7 => alu_r(opcode, Instruction::And),
        0xA8..=0xAF => alu_r(opcode, Instruction::Xor),
        0xB0..=0xB7 => alu_r(opcode, Instruction::Or),
        0xB8..=0xBF => alu_r(opcode, Instruction::Cp),

        0xE2 => ok(Instruction::LdIoCA, 1),
        0xE9 => ok(Instruction::JpHl, 1),
        0xF2 => ok(Instruction::LdAIoC, 1),
        0xF3 => ok(Instruction::Di, 1),
        0xFB => ok(Instruction::Ei, 1),

        // LD (rr),A / LD A,(rr)
        0x02 => ok(Instruction::LdMemBcA, 1),
        0x0A => ok(Instruction::LdAMemBc, 1),
        0x12 => ok(Instruction::LdMemDeA, 1),
        0x1A => ok(Instruction::LdAMemDe, 1),

        // ADD HL,rr
        0x09 => ok(Instruction::AddHl(Reg16::Bc), 1),
        0x19 => ok(Instruction::AddHl(Reg16::De), 1),
        0x29 => ok(Instruction::AddHl(Reg16::Hl), 1),
        0x39 => ok(Instruction::AddHl(Reg16::Sp), 1),

        // INC/DEC rr
        0x03 => ok(Instruction::Inc16(Reg16::Bc), 1),
        0x13 => ok(Instruction::Inc16(Reg16::De), 1),
        0x23 => ok(Instruction::Inc16(Reg16::Hl), 1),
        0x33 => ok(Instruction::Inc16(Reg16::Sp), 1),
        0x0B => ok(Instruction::Dec16(Reg16::Bc), 1),
        0x1B => ok(Instruction::Dec16(Reg16::De), 1),
        0x2B => ok(Instruction::Dec16(Reg16::Hl), 1),
        0x3B => ok(Instruction::Dec16(Reg16::Sp), 1),

        // INC/DEC r8 (and (HL))
        0x04 => ok(Instruction::Inc8(Operand8::Reg(Reg8::B)), 1),
        0x0C => ok(Instruction::Inc8(Operand8::Reg(Reg8::C)), 1),
        0x14 => ok(Instruction::Inc8(Operand8::Reg(Reg8::D)), 1),
        0x1C => ok(Instruction::Inc8(Operand8::Reg(Reg8::E)), 1),
        0x24 => ok(Instruction::Inc8(Operand8::Reg(Reg8::H)), 1),
        0x2C => ok(Instruction::Inc8(Operand8::Reg(Reg8::L)), 1),
        0x34 => ok(Instruction::Inc8(Operand8::MemHl), 1),
        0x3C => ok(Instruction::Inc8(Operand8::Reg(Reg8::A)), 1),
        0x05 => ok(Instruction::Dec8(Operand8::Reg(Reg8::B)), 1),
        0x0D => ok(Instruction::Dec8(Operand8::Reg(Reg8::C)), 1),
        0x15 => ok(Instruction::Dec8(Operand8::Reg(Reg8::D)), 1),
        0x1D => ok(Instruction::Dec8(Operand8::Reg(Reg8::E)), 1),
        0x25 => ok(Instruction::Dec8(Operand8::Reg(Reg8::H)), 1),
        0x2D => ok(Instruction::Dec8(Operand8::Reg(Reg8::L)), 1),
        0x35 => ok(Instruction::Dec8(Operand8::MemHl), 1),
        0x3D => ok(Instruction::Dec8(Operand8::Reg(Reg8::A)), 1),

        // LD (HL±),A / LD A,(HL±)
        0x22 => ok(Instruction::LdiMemHlA, 1),
        0x2A => ok(Instruction::LdiAMemHl, 1),
        0x32 => ok(Instruction::LddMemHlA, 1),
        0x3A => ok(Instruction::LddAMemHl, 1),

        // LD r,r / LD r,(HL) / LD (HL),r (excludes HALT at $76)
        0x40..=0x75 | 0x77..=0x7F => ld_r_r(opcode),

        // RET / conditional RET
        0xC0 => ok(
            Instruction::Ret {
                cond: Some(Cond::Nz),
            },
            1,
        ),
        0xC8 => ok(
            Instruction::Ret {
                cond: Some(Cond::Z),
            },
            1,
        ),
        0xC9 => ok(Instruction::Ret { cond: None }, 1),
        0xD0 => ok(
            Instruction::Ret {
                cond: Some(Cond::Nc),
            },
            1,
        ),
        0xD8 => ok(
            Instruction::Ret {
                cond: Some(Cond::C),
            },
            1,
        ),
        0xD9 => ok(Instruction::Reti, 1),

        // PUSH / POP
        0xC1 => ok(Instruction::Pop(Reg16::Bc), 1),
        0xC5 => ok(Instruction::Push(Reg16::Bc), 1),
        0xD1 => ok(Instruction::Pop(Reg16::De), 1),
        0xD5 => ok(Instruction::Push(Reg16::De), 1),
        0xE1 => ok(Instruction::Pop(Reg16::Hl), 1),
        0xE5 => ok(Instruction::Push(Reg16::Hl), 1),
        0xF1 => ok(Instruction::Pop(Reg16::Af), 1),
        0xF5 => ok(Instruction::Push(Reg16::Af), 1),

        // --- d8 ---
        0x06 => ld_r8_d8(bytes, Reg8::B),
        0x0E => ld_r8_d8(bytes, Reg8::C),
        0x16 => ld_r8_d8(bytes, Reg8::D),
        0x1E => ld_r8_d8(bytes, Reg8::E),
        0x26 => ld_r8_d8(bytes, Reg8::H),
        0x2E => ld_r8_d8(bytes, Reg8::L),
        0x36 => ld_mem_hl_d8(bytes),
        0x3E => ld_r8_d8(bytes, Reg8::A),
        0xE0 => imm8(bytes, Instruction::LdhAbsA),
        0xC6 => alu_imm(bytes, Instruction::Add),
        0xCE => alu_imm(bytes, Instruction::Adc),
        0xD6 => alu_imm(bytes, Instruction::Sub),
        0xDE => alu_imm(bytes, Instruction::Sbc),
        0xE6 => alu_imm(bytes, Instruction::And),
        0xEE => alu_imm(bytes, Instruction::Xor),
        0xF0 => imm8(bytes, Instruction::LdhAAbs),
        0xF6 => alu_imm(bytes, Instruction::Or),
        0xFE => alu_imm(bytes, Instruction::Cp),

        // --- d16 (LD rr,d16) ---
        0x01 => ld16_imm(bytes, Reg16::Bc),
        0x11 => ld16_imm(bytes, Reg16::De),
        0x21 => ld16_imm(bytes, Reg16::Hl),
        0x31 => ld16_imm(bytes, Reg16::Sp),

        // --- JR ---
        0x18 => jr(bytes, None),
        0x20 => jr(bytes, Some(Cond::Nz)),
        0x28 => jr(bytes, Some(Cond::Z)),
        0x30 => jr(bytes, Some(Cond::Nc)),
        0x38 => jr(bytes, Some(Cond::C)),

        // --- JP / CALL a16 ---
        0xC2 => jp(bytes, Some(Cond::Nz)),
        0xC3 => jp(bytes, None),
        0xC4 => call(bytes, Some(Cond::Nz)),
        0xCA => jp(bytes, Some(Cond::Z)),
        0xCC => call(bytes, Some(Cond::Z)),
        0xCD => call(bytes, None),
        0xD2 => jp(bytes, Some(Cond::Nc)),
        0xD4 => call(bytes, Some(Cond::Nc)),
        0xDA => jp(bytes, Some(Cond::C)),
        0xDC => call(bytes, Some(Cond::C)),

        // --- RST n ---
        0xC7 => ok(Instruction::Rst(0x00), 1),
        0xCF => ok(Instruction::Rst(0x08), 1),
        0xD7 => ok(Instruction::Rst(0x10), 1),
        0xDF => ok(Instruction::Rst(0x18), 1),
        0xE7 => ok(Instruction::Rst(0x20), 1),
        0xEF => ok(Instruction::Rst(0x28), 1),
        0xF7 => ok(Instruction::Rst(0x30), 1),
        0xFF => ok(Instruction::Rst(0x38), 1),

        0xEA => imm16(bytes, Instruction::LdAbsA),
        0xFA => imm16(bytes, Instruction::LdAAbs),
        0xE8 => {
            let e = *bytes.get(1)? as i8;
            ok(Instruction::AddSpE(e), 2)
        }
        0xF8 => {
            let e = *bytes.get(1)? as i8;
            ok(Instruction::LdHlSpE(e), 2)
        }
        0xF9 => ok(Instruction::LdSpHl, 1),

        // --- CB ---
        0xCB => decode_cb(bytes),

        _ => None,
    }
}

fn ok(instruction: Instruction, len: u8) -> Option<Decoded> {
    Some(Decoded { instruction, len })
}

fn ld_r_r(opcode: u8) -> Option<Decoded> {
    let dst_bits = (opcode >> 3) & 0b111;
    let src_bits = opcode & 0b111;
    let dst = match dst_bits {
        6 => Operand8::MemHl,
        _ => Operand8::Reg(r8_from_bits(dst_bits)?),
    };
    let src = match src_bits {
        6 => Operand8::MemHl,
        _ => Operand8::Reg(r8_from_bits(src_bits)?),
    };
    // HALT is $76 (LD (HL),(HL)); already excluded by the match arms.
    ok(Instruction::Ld8 { dst, src }, 1)
}

fn alu_r(opcode: u8, make: impl FnOnce(Operand8) -> Instruction) -> Option<Decoded> {
    ok(make(operand8_from_bits(opcode)?), 1)
}

fn operand8_from_bits(bits: u8) -> Option<Operand8> {
    match bits & 0b111 {
        6 => Some(Operand8::MemHl),
        n => Some(Operand8::Reg(r8_from_bits(n)?)),
    }
}

fn r8_from_bits(bits: u8) -> Option<Reg8> {
    match bits & 0b111 {
        0 => Some(Reg8::B),
        1 => Some(Reg8::C),
        2 => Some(Reg8::D),
        3 => Some(Reg8::E),
        4 => Some(Reg8::H),
        5 => Some(Reg8::L),
        6 => None, // (HL)
        7 => Some(Reg8::A),
        _ => None,
    }
}

fn ld_r8_d8(bytes: &[u8], dst: Reg8) -> Option<Decoded> {
    let n = *bytes.get(1)?;
    ok(
        Instruction::Ld8 {
            dst: Operand8::Reg(dst),
            src: Operand8::Imm(n),
        },
        2,
    )
}

fn ld_mem_hl_d8(bytes: &[u8]) -> Option<Decoded> {
    let n = *bytes.get(1)?;
    ok(
        Instruction::Ld8 {
            dst: Operand8::MemHl,
            src: Operand8::Imm(n),
        },
        2,
    )
}

fn alu_imm(bytes: &[u8], make: impl FnOnce(Operand8) -> Instruction) -> Option<Decoded> {
    let n = *bytes.get(1)?;
    ok(make(Operand8::Imm(n)), 2)
}

fn imm8(bytes: &[u8], make: impl FnOnce(u8) -> Instruction) -> Option<Decoded> {
    let n = *bytes.get(1)?;
    ok(make(n), 2)
}

fn imm16(bytes: &[u8], make: impl FnOnce(u16) -> Instruction) -> Option<Decoded> {
    let addr = read_u16(bytes)?;
    ok(make(addr), 3)
}

fn read_u16(bytes: &[u8]) -> Option<u16> {
    let lo = *bytes.get(1)?;
    let hi = *bytes.get(2)?;
    Some(u16::from_le_bytes([lo, hi]))
}

fn jr(bytes: &[u8], cond: Option<Cond>) -> Option<Decoded> {
    let offset = *bytes.get(1)? as i8;
    ok(Instruction::Jr { cond, offset }, 2)
}

fn jp(bytes: &[u8], cond: Option<Cond>) -> Option<Decoded> {
    let target = read_u16(bytes)?;
    ok(Instruction::Jp { cond, target }, 3)
}

fn call(bytes: &[u8], cond: Option<Cond>) -> Option<Decoded> {
    let target = read_u16(bytes)?;
    ok(Instruction::Call { cond, target }, 3)
}

fn ld16_imm(bytes: &[u8], dst: Reg16) -> Option<Decoded> {
    let value = read_u16(bytes)?;
    ok(Instruction::Ld16Imm { dst, value }, 3)
}

fn decode_cb(bytes: &[u8]) -> Option<Decoded> {
    let cb = *bytes.get(1)?;
    match cb {
        0x00..=0x07 => ok(Instruction::Rlc(operand8_from_bits(cb)?), 2),
        0x08..=0x0F => ok(Instruction::Rrc(operand8_from_bits(cb)?), 2),
        0x10..=0x17 => ok(Instruction::Rl(operand8_from_bits(cb)?), 2),
        0x18..=0x1F => ok(Instruction::Rr(operand8_from_bits(cb)?), 2),
        0x20..=0x27 => ok(Instruction::Sla(operand8_from_bits(cb)?), 2),
        0x28..=0x2F => ok(Instruction::Sra(operand8_from_bits(cb)?), 2),
        0x30..=0x37 => ok(Instruction::Swap(operand8_from_bits(cb)?), 2),
        0x38..=0x3F => ok(Instruction::Srl(operand8_from_bits(cb)?), 2),
        0x40..=0x7F => {
            let op = operand8_from_bits(cb)?;
            let bit = (cb >> 3) & 0b111;
            ok(Instruction::Bit { bit, op }, 2)
        }
        0x80..=0xBF => {
            let op = operand8_from_bits(cb)?;
            let bit = (cb >> 3) & 0b111;
            ok(Instruction::Res { bit, op }, 2)
        }
        0xC0..=0xFF => {
            let op = operand8_from_bits(cb)?;
            let bit = (cb >> 3) & 0b111;
            ok(Instruction::Set { bit, op }, 2)
        }
    }
}

#[cfg(test)]
mod tests;

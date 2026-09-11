use super::*;
use crate::cpu::jr_target;

#[test]
fn decodes_nop() {
    let d = decode(&[0x00]).unwrap();
    assert_eq!(d.len, 1);
    assert_eq!(d.instruction, Instruction::Nop);
}

#[test]
fn decodes_jp_and_call() {
    assert_eq!(
        decode(&[0xC3, 0x50, 0x01]).unwrap().instruction,
        Instruction::Jp {
            cond: None,
            target: 0x0150
        }
    );
    assert_eq!(
        decode(&[0xCA, 0x34, 0x12]).unwrap().instruction,
        Instruction::Jp {
            cond: Some(Cond::Z),
            target: 0x1234
        }
    );
    assert_eq!(
        decode(&[0xCD, 0x00, 0x20]).unwrap().instruction,
        Instruction::Call {
            cond: None,
            target: 0x2000
        }
    );
}

#[test]
fn decodes_rst_family() {
    let cases = [
        (0xC7u8, 0x00u8),
        (0xCF, 0x08),
        (0xD7, 0x10),
        (0xDF, 0x18),
        (0xE7, 0x20),
        (0xEF, 0x28),
        (0xF7, 0x30),
        (0xFF, 0x38),
    ];
    for (op, vec) in cases {
        let d = decode(&[op]).unwrap();
        assert_eq!(d.len, 1);
        assert_eq!(d.instruction, Instruction::Rst(vec));
        assert_eq!(d.instruction.mnemonic(0), format!("RST ${vec:02X}"));
    }
}

#[test]
fn decodes_conditional_call_family() {
    assert_eq!(
        decode(&[0xC4, 0x34, 0x12]).unwrap().instruction,
        Instruction::Call {
            cond: Some(Cond::Nz),
            target: 0x1234
        }
    );
    assert_eq!(
        decode(&[0xCC, 0x34, 0x12]).unwrap().instruction,
        Instruction::Call {
            cond: Some(Cond::Z),
            target: 0x1234
        }
    );
    assert_eq!(
        decode(&[0xD4, 0x34, 0x12]).unwrap().instruction,
        Instruction::Call {
            cond: Some(Cond::Nc),
            target: 0x1234
        }
    );
    assert_eq!(
        decode(&[0xDC, 0x34, 0x12]).unwrap().instruction,
        Instruction::Call {
            cond: Some(Cond::C),
            target: 0x1234
        }
    );
    assert_eq!(
        decode(&[0xCC, 0x5F, 0x01]).unwrap().instruction.mnemonic(0),
        "CALL Z, $015F"
    );
}

#[test]
fn decodes_ret_family() {
    assert_eq!(
        decode(&[0xC9]).unwrap().instruction,
        Instruction::Ret { cond: None }
    );
    assert_eq!(
        decode(&[0xC0]).unwrap().instruction,
        Instruction::Ret {
            cond: Some(Cond::Nz)
        }
    );
    assert_eq!(
        decode(&[0xC8]).unwrap().instruction,
        Instruction::Ret {
            cond: Some(Cond::Z)
        }
    );
    assert_eq!(decode(&[0xC9]).unwrap().instruction.mnemonic(0), "RET");
    assert_eq!(decode(&[0xC8]).unwrap().instruction.mnemonic(0), "RET Z");
    assert_eq!(decode(&[0xD9]).unwrap().instruction, Instruction::Reti);
    assert_eq!(decode(&[0xD9]).unwrap().instruction.mnemonic(0), "RETI");
}

#[test]
fn jr_z_positive_target() {
    let d = decode(&[0x28, 0x03]).unwrap();
    assert_eq!(
        d.instruction,
        Instruction::Jr {
            cond: Some(Cond::Z),
            offset: 3
        }
    );
    assert_eq!(jr_target(0x0152, 3), 0x0157);
    assert_eq!(d.instruction.mnemonic(0x0152), "JR Z, $0157");
}

#[test]
fn jr_negative_offset() {
    let d = decode(&[0x18, 0xFA]).unwrap();
    assert_eq!(
        d.instruction,
        Instruction::Jr {
            cond: None,
            offset: -6
        }
    );
    assert_eq!(jr_target(0x0200, -6), 0x01FC);
    assert_eq!(d.instruction.mnemonic(0x0200), "JR $01FC");
}

#[test]
fn decodes_ld_and_alu() {
    assert_eq!(
        decode(&[0x3E, 0x20]).unwrap().instruction,
        Instruction::Ld8 {
            dst: Operand8::Reg(Reg8::A),
            src: Operand8::Imm(0x20)
        }
    );
    assert_eq!(
        decode(&[0xEA, 0x1A, 0xCF]).unwrap().instruction,
        Instruction::LdAbsA(0xCF1A)
    );
    assert_eq!(
        decode(&[0xB0]).unwrap().instruction,
        Instruction::Or(Operand8::Reg(Reg8::B))
    );
    assert_eq!(
        decode(&[0xCB, 0x37]).unwrap().instruction,
        Instruction::Swap(Operand8::Reg(Reg8::A))
    );
    assert_eq!(
        decode(&[0xCB, 0x1A]).unwrap().instruction,
        Instruction::Rr(Operand8::Reg(Reg8::D))
    );
    assert_eq!(
        decode(&[0xCB, 0x16]).unwrap().instruction,
        Instruction::Rl(Operand8::MemHl)
    );
    assert_eq!(
        decode(&[0xCB, 0x20]).unwrap().instruction,
        Instruction::Sla(Operand8::Reg(Reg8::B))
    );
    assert_eq!(
        decode(&[0xCB, 0x2E]).unwrap().instruction,
        Instruction::Sra(Operand8::MemHl)
    );
    assert_eq!(
        decode(&[0xCB, 0x3F]).unwrap().instruction,
        Instruction::Srl(Operand8::Reg(Reg8::A))
    );
    assert_eq!(
        decode(&[0xCB, 0x3F]).unwrap().instruction.mnemonic(0),
        "SRL A"
    );
    assert_eq!(
        decode(&[0xCB, 0x87]).unwrap().instruction,
        Instruction::Res {
            bit: 0,
            op: Operand8::Reg(Reg8::A)
        }
    );
    assert_eq!(
        decode(&[0xCB, 0xAE]).unwrap().instruction,
        Instruction::Res {
            bit: 5,
            op: Operand8::MemHl
        }
    );
    assert_eq!(
        decode(&[0xCB, 0xDE]).unwrap().instruction,
        Instruction::Set {
            bit: 3,
            op: Operand8::MemHl
        }
    );
    assert_eq!(
        decode(&[0xCB, 0x42]).unwrap().instruction,
        Instruction::Bit {
            bit: 0,
            op: Operand8::Reg(Reg8::D)
        }
    );
    assert_eq!(
        decode(&[0xCB, 0x46]).unwrap().instruction,
        Instruction::Bit {
            bit: 0,
            op: Operand8::MemHl
        }
    );
    assert_eq!(
        decode(&[0x78]).unwrap().instruction,
        Instruction::Ld8 {
            dst: Operand8::Reg(Reg8::A),
            src: Operand8::Reg(Reg8::B)
        }
    );
    assert_eq!(
        decode(&[0x36, 0x00]).unwrap().instruction,
        Instruction::Ld8 {
            dst: Operand8::MemHl,
            src: Operand8::Imm(0x00)
        }
    );
    assert_eq!(
        decode(&[0x23]).unwrap().instruction,
        Instruction::Inc16(Reg16::Hl)
    );
    assert_eq!(
        decode(&[0x0B]).unwrap().instruction,
        Instruction::Dec16(Reg16::Bc)
    );
    assert_eq!(
        decode(&[0xB1]).unwrap().instruction,
        Instruction::Or(Operand8::Reg(Reg8::C))
    );
    assert_eq!(
        decode(&[0x87]).unwrap().instruction,
        Instruction::Add(Operand8::Reg(Reg8::A))
    );
    assert_eq!(
        decode(&[0x8E]).unwrap().instruction,
        Instruction::Adc(Operand8::MemHl)
    );
    assert_eq!(decode(&[0xE9]).unwrap().instruction, Instruction::JpHl);
    assert_eq!(decode(&[0xE9]).unwrap().instruction.mnemonic(0), "JP (HL)");
    assert_eq!(decode(&[0x07]).unwrap().instruction, Instruction::Rlca);
    assert_eq!(decode(&[0x0F]).unwrap().instruction, Instruction::Rrca);
    assert_eq!(decode(&[0x17]).unwrap().instruction, Instruction::Rla);
    assert_eq!(decode(&[0x1F]).unwrap().instruction, Instruction::Rra);
    assert_eq!(decode(&[0x27]).unwrap().instruction, Instruction::Daa);
    assert_eq!(
        decode(&[0x08, 0x34, 0x12]).unwrap().instruction,
        Instruction::LdAbsSp(0x1234)
    );
    assert_eq!(decode(&[0x07]).unwrap().instruction.mnemonic(0), "RLCA");
}

#[test]
fn truncated_and_unknown() {
    assert!(decode(&[0xCE]).is_none()); // ADC A,d8 needs immediate
    assert!(decode(&[0xCB, 0xD0]).is_some()); // SET is covered
    assert!(decode(&[]).is_none());
    assert!(decode(&[0xC3]).is_none());
    assert!(decode(&[0xC3, 0x50]).is_none());
    assert!(decode(&[0xCD, 0x00]).is_none());
    assert!(decode(&[0xF8]).is_none());
    assert!(decode(&[0xE8]).is_none());
    assert!(decode(&[0xCB]).is_none());
}

#[test]
fn decodes_push_pop_and_ld16() {
    assert_eq!(
        decode(&[0xF5]).unwrap().instruction,
        Instruction::Push(Reg16::Af)
    );
    assert_eq!(
        decode(&[0xC1]).unwrap().instruction,
        Instruction::Pop(Reg16::Bc)
    );
    assert_eq!(
        decode(&[0x21, 0x4D, 0x01]).unwrap().instruction,
        Instruction::Ld16Imm {
            dst: Reg16::Hl,
            value: 0x014D
        }
    );
    assert_eq!(
        decode(&[0x31, 0xFE, 0xFF]).unwrap().instruction,
        Instruction::Ld16Imm {
            dst: Reg16::Sp,
            value: 0xFFFE
        }
    );
    assert_eq!(
        decode(&[0xF8, 0x02]).unwrap().instruction,
        Instruction::LdHlSpE(2)
    );
    assert_eq!(
        decode(&[0xF8, 0xFE]).unwrap().instruction,
        Instruction::LdHlSpE(-2)
    );
    assert_eq!(
        decode(&[0xF8, 0x00]).unwrap().instruction.mnemonic(0),
        "LD HL, SP+0"
    );
    assert_eq!(decode(&[0xF9]).unwrap().instruction, Instruction::LdSpHl);
    assert_eq!(
        decode(&[0xE8, 0x02]).unwrap().instruction,
        Instruction::AddSpE(2)
    );
    assert_eq!(
        decode(&[0xE8, 0xFE]).unwrap().instruction,
        Instruction::AddSpE(-2)
    );
    assert_eq!(
        decode(&[0xE8, 0x01]).unwrap().instruction.mnemonic(0),
        "ADD SP, +1"
    );
}

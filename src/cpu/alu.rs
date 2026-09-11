//! Pure 8/16-bit ALU helpers (value + flags). No CPU mutation.

/// Result of an 8-bit arithmetic/logic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluResult8 {
    pub value: u8,
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

/// Result of `INC` / `DEC` (carry flag is unchanged by those ops).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncDecResult8 {
    pub value: u8,
    pub z: bool,
    pub n: bool,
    pub h: bool,
}

/// Result of `ADD HL, rr` (Z unchanged on hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddHlResult {
    pub value: u16,
    pub h: bool,
    pub c: bool,
}

pub fn add8(a: u8, b: u8) -> AluResult8 {
    adc8(a, b, false)
}

pub fn adc8(a: u8, b: u8, carry: bool) -> AluResult8 {
    let c_in = u8::from(carry);
    let sum = u16::from(a) + u16::from(b) + u16::from(c_in);
    let value = sum as u8;
    AluResult8 {
        value,
        z: value == 0,
        n: false,
        h: (a & 0x0F) + (b & 0x0F) + c_in > 0x0F,
        c: sum > 0xFF,
    }
}

pub fn sub8(a: u8, b: u8) -> AluResult8 {
    sbc8(a, b, false)
}

pub fn sbc8(a: u8, b: u8, carry: bool) -> AluResult8 {
    let c_in = u8::from(carry);
    let diff = u16::from(a)
        .wrapping_sub(u16::from(b))
        .wrapping_sub(u16::from(c_in));
    let value = diff as u8;
    AluResult8 {
        value,
        z: value == 0,
        n: true,
        h: u16::from(a & 0x0F) < u16::from(b & 0x0F) + u16::from(c_in),
        c: u16::from(a) < u16::from(b) + u16::from(c_in),
    }
}

pub fn and8(a: u8, b: u8) -> AluResult8 {
    let value = a & b;
    AluResult8 {
        value,
        z: value == 0,
        n: false,
        h: true,
        c: false,
    }
}

pub fn xor8(a: u8, b: u8) -> AluResult8 {
    let value = a ^ b;
    AluResult8 {
        value,
        z: value == 0,
        n: false,
        h: false,
        c: false,
    }
}

pub fn or8(a: u8, b: u8) -> AluResult8 {
    let value = a | b;
    AluResult8 {
        value,
        z: value == 0,
        n: false,
        h: false,
        c: false,
    }
}

/// `CP` is `SUB` without writing the result.
pub fn cp8(a: u8, b: u8) -> AluResult8 {
    sub8(a, b)
}

/// Decimal adjust accumulator after BCD add/sub ([gbz80(7)](https://manpages.ubuntu.com/manpages/resolute/man7/gbz80.7.html)).
pub fn daa(a: u8, n: bool, h: bool, c_in: bool) -> AluResult8 {
    let mut adjust = 0u8;
    let mut c = c_in;
    if !n {
        if h || (a & 0x0F) > 0x09 {
            adjust |= 0x06;
        }
        if c_in || a > 0x99 {
            adjust |= 0x60;
            c = true;
        }
        let value = a.wrapping_add(adjust);
        AluResult8 {
            value,
            z: value == 0,
            n: false,
            h: false,
            c,
        }
    } else {
        if h {
            adjust |= 0x06;
        }
        if c_in {
            adjust |= 0x60;
        }
        let value = a.wrapping_sub(adjust);
        AluResult8 {
            value,
            z: value == 0,
            n: true,
            h: false,
            c: c_in,
        }
    }
}

pub fn inc8(a: u8) -> IncDecResult8 {
    let value = a.wrapping_add(1);
    IncDecResult8 {
        value,
        z: value == 0,
        n: false,
        h: (a & 0x0F) == 0x0F,
    }
}

pub fn dec8(a: u8) -> IncDecResult8 {
    let value = a.wrapping_sub(1);
    IncDecResult8 {
        value,
        z: value == 0,
        n: true,
        h: (a & 0x0F) == 0x00,
    }
}

pub fn add_hl(hl: u16, rr: u16) -> AddHlResult {
    let (value, c) = hl.overflowing_add(rr);
    AddHlResult {
        value,
        h: ((hl & 0x0FFF) + (rr & 0x0FFF)) > 0x0FFF,
        c,
    }
}

/// Result of `ADD SP, e8` / `LD HL, SP+e8` (Z and N are cleared by the instruction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpOffsetResult {
    pub value: u16,
    pub h: bool,
    pub c: bool,
}

/// `SP + signed e8` with H/C from the low-byte unsigned add (Pan Docs).
pub fn add_sp_e8(sp: u16, e: i8) -> SpOffsetResult {
    let e_u = e as u8;
    let value = sp.wrapping_add(e as i16 as u16);
    SpOffsetResult {
        value,
        h: (sp & 0x0F) + (u16::from(e_u) & 0x0F) > 0x0F,
        c: (sp & 0xFF) + u16::from(e_u) > 0xFF,
    }
}

/// `BIT b,r` flag result. Carry is unchanged by the instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitFlags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
}

/// Test whether `bit` (0–7) is clear in `value` → Z; always N=0, H=1.
pub fn bit(value: u8, bit: u8) -> BitFlags {
    let bit_set = value & (1 << (bit & 7)) != 0;
    BitFlags {
        z: !bit_set,
        n: false,
        h: true,
    }
}

fn rotate_flags(value: u8, carry: bool) -> AluResult8 {
    AluResult8 {
        value,
        z: value == 0,
        n: false,
        h: false,
        c: carry,
    }
}

/// Rotate left circular: bit7 → bit0 and into carry.
pub fn rlc(value: u8) -> AluResult8 {
    let carry = value & 0x80 != 0;
    let result = value.rotate_left(1);
    rotate_flags(result, carry)
}

/// Rotate right circular: bit0 → bit7 and into carry.
pub fn rrc(value: u8) -> AluResult8 {
    let carry = value & 0x01 != 0;
    let result = value.rotate_right(1);
    rotate_flags(result, carry)
}

/// Rotate left through carry.
pub fn rl(value: u8, carry_in: bool) -> AluResult8 {
    let carry_out = value & 0x80 != 0;
    let result = (value << 1) | u8::from(carry_in);
    rotate_flags(result, carry_out)
}

/// Rotate right through carry.
pub fn rr(value: u8, carry_in: bool) -> AluResult8 {
    let carry_out = value & 0x01 != 0;
    let result = (value >> 1) | (u8::from(carry_in) << 7);
    rotate_flags(result, carry_out)
}

/// Shift left arithmetic: bit0 ← 0, C ← old bit7.
pub fn sla(value: u8) -> AluResult8 {
    let carry = value & 0x80 != 0;
    rotate_flags(value << 1, carry)
}

/// Shift right arithmetic: bit7 preserved, C ← old bit0.
pub fn sra(value: u8) -> AluResult8 {
    let carry = value & 0x01 != 0;
    rotate_flags((value >> 1) | (value & 0x80), carry)
}

/// Shift right logical: bit7 ← 0, C ← old bit0.
pub fn srl(value: u8) -> AluResult8 {
    let carry = value & 0x01 != 0;
    rotate_flags(value >> 1, carry)
}

#[cfg(test)]
mod tests;

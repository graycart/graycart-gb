//! CPU registers and flags ([Pan Docs](https://gbdev.io/pandocs/CPU_Registers_and_Flags.html)).
//!
//! | 16-bit | Hi | Lo |
//! |--------|----|----|
//! | AF     | A  | F  |
//! | BC     | B  | C  |
//! | DE     | D  | E  |
//! | HL     | H  | L  |
//! | SP     | —  | —  |
//! | PC     | —  | —  |
//!
//! Flag register `F` (low byte of AF):
//!
//! | Bit | Name | Meaning              |
//! |-----|------|----------------------|
//! | 7   | Z    | Zero                 |
//! | 6   | N    | Subtract (BCD / DAA) |
//! | 5   | H    | Half-carry (BCD)     |
//! | 4   | C    | Carry                |
//! | 3–0 | —    | Unused; always 0     |

/// Flag bits in `F` (upper nibble only; bits 0–3 are always 0).
pub const FLAG_Z: u8 = 0b1000_0000;
pub const FLAG_N: u8 = 0b0100_0000;
pub const FLAG_H: u8 = 0b0010_0000;
pub const FLAG_C: u8 = 0b0001_0000;

/// Mask for the usable flag bits (ZNHC). Lower nibble of `F` is unused.
const FLAG_MASK: u8 = 0xF0;

/// LR35902 CPU register file + PC/SP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    /// Flags; only bits 7–4 are meaningful.
    f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    /// Interrupt master enable.
    pub ime: bool,
    /// Set by `EI`; becomes `ime_enable_armed` after this instruction ends.
    pub ime_enable_pending: bool,
    /// When set, `IME` turns on after the *current* instruction finishes.
    pub ime_enable_armed: bool,
    /// Set by `HALT`; cleared when an interrupt wakes the CPU.
    pub halted: bool,
}

impl Cpu {
    /// Zeroed registers (useful before explicit setup).
    pub fn new() -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
            ime: false,
            ime_enable_pending: false,
            ime_enable_armed: false,
            halted: false,
        }
    }

    /// DMG register values after the boot ROM hands off (entry at `$0100`).
    pub fn after_boot() -> Self {
        Self {
            a: 0x01,
            f: 0xB0, // Z=1 N=0 H=1 C=1
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_enable_pending: false,
            ime_enable_armed: false,
            halted: false,
        }
    }

    /// Native CGB register values after Fast skip / boot handoff (entry at `$0100`).
    ///
    /// Pan Docs Power-Up Sequence, CGB column: A=`$11`, F=`$80`, DE=`$FF56`, HL=`$000D`.
    pub fn after_boot_cgb() -> Self {
        Self {
            a: 0x11,
            f: 0x80, // Z=1 N=0 H=0 C=0
            b: 0x00,
            c: 0x00,
            d: 0xFF,
            e: 0x56,
            h: 0x00,
            l: 0x0D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_enable_pending: false,
            ime_enable_armed: false,
            halted: false,
        }
    }

    /// CGB running a DMG cart (compat mode) after Fast skip / boot handoff.
    ///
    /// Pan Docs: A=`$11`, F=`$80`, DE=`$0008`, HL=`$007C` (when B is not `$43`/`$58`).
    pub fn after_boot_cgb_compat() -> Self {
        Self {
            a: 0x11,
            f: 0x80, // Z=1 N=0 H=0 C=0
            b: 0x00,
            c: 0x00,
            d: 0x00,
            e: 0x08,
            h: 0x00,
            l: 0x7C,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_enable_pending: false,
            ime_enable_armed: false,
            halted: false,
        }
    }

    pub fn f(&self) -> u8 {
        self.f & FLAG_MASK
    }

    pub fn set_f(&mut self, value: u8) {
        self.f = value & FLAG_MASK;
    }

    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f()])
    }

    pub fn set_af(&mut self, value: u16) {
        let [a, f] = value.to_be_bytes();
        self.a = a;
        self.set_f(f);
    }

    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    pub fn set_bc(&mut self, value: u16) {
        let [b, c] = value.to_be_bytes();
        self.b = b;
        self.c = c;
    }

    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    pub fn set_de(&mut self, value: u16) {
        let [d, e] = value.to_be_bytes();
        self.d = d;
        self.e = e;
    }

    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    pub fn set_hl(&mut self, value: u16) {
        let [h, l] = value.to_be_bytes();
        self.h = h;
        self.l = l;
    }

    pub fn read_r8(&self, reg: super::Reg8) -> u8 {
        use super::Reg8::*;
        match reg {
            A => self.a,
            B => self.b,
            C => self.c,
            D => self.d,
            E => self.e,
            H => self.h,
            L => self.l,
        }
    }

    pub fn write_r8(&mut self, reg: super::Reg8, value: u8) {
        use super::Reg8::*;
        match reg {
            A => self.a = value,
            B => self.b = value,
            C => self.c = value,
            D => self.d = value,
            E => self.e = value,
            H => self.h = value,
            L => self.l = value,
        }
    }

    pub fn read_r16(&self, reg: super::Reg16) -> u16 {
        use super::Reg16::*;
        match reg {
            Af => self.af(),
            Bc => self.bc(),
            De => self.de(),
            Hl => self.hl(),
            Sp => self.sp,
        }
    }

    pub fn write_r16(&mut self, reg: super::Reg16, value: u16) {
        use super::Reg16::*;
        match reg {
            Af => self.set_af(value),
            Bc => self.set_bc(value),
            De => self.set_de(value),
            Hl => self.set_hl(value),
            Sp => self.sp = value,
        }
    }

    pub fn flag_z(&self) -> bool {
        self.f() & FLAG_Z != 0
    }

    pub fn flag_n(&self) -> bool {
        self.f() & FLAG_N != 0
    }

    pub fn flag_h(&self) -> bool {
        self.f() & FLAG_H != 0
    }

    pub fn flag_c(&self) -> bool {
        self.f() & FLAG_C != 0
    }

    pub fn set_flag_z(&mut self, on: bool) {
        self.set_flag(FLAG_Z, on);
    }

    pub fn set_flag_n(&mut self, on: bool) {
        self.set_flag(FLAG_N, on);
    }

    pub fn set_flag_h(&mut self, on: bool) {
        self.set_flag(FLAG_H, on);
    }

    pub fn set_flag_c(&mut self, on: bool) {
        self.set_flag(FLAG_C, on);
    }

    fn set_flag(&mut self, mask: u8, on: bool) {
        if on {
            self.f |= mask;
        } else {
            self.f &= !mask;
        }
        self.f &= FLAG_MASK;
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::CpuStateV1 {
        crate::snapshot::CpuStateV1 {
            a: self.a,
            f: self.f(),
            b: self.b,
            c: self.c,
            d: self.d,
            e: self.e,
            h: self.h,
            l: self.l,
            sp: self.sp,
            pc: self.pc,
            ime: self.ime,
            ime_enable_pending: self.ime_enable_pending,
            ime_enable_armed: self.ime_enable_armed,
            halted: self.halted,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, state: &crate::snapshot::CpuStateV1) {
        self.a = state.a;
        self.set_f(state.f);
        self.b = state.b;
        self.c = state.c;
        self.d = state.d;
        self.e = state.e;
        self.h = state.h;
        self.l = state.l;
        self.sp = state.sp;
        self.pc = state.pc;
        self.ime = state.ime;
        self.ime_enable_pending = state.ime_enable_pending;
        self.ime_enable_armed = state.ime_enable_armed;
        self.halted = state.halted;
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Cpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PC={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X}",
            self.pc,
            self.af(),
            self.bc(),
            self.de(),
            self.hl(),
            self.sp
        )
    }
}

#[cfg(test)]
mod tests;

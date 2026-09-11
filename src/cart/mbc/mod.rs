//! Memory bank controllers ([Pan Docs](https://gbdev.io/pandocs/MBCs.html)).
//!
//! Mapper implementations are driven by documented hardware behavior and test
//! coverage. Unsupported cartridge types must fail clearly rather than silently
//! behaving like ROM-only.

mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;
mod none;
pub(crate) mod rtc;

pub(crate) use mbc1::Mbc1;
pub(crate) use mbc2::{MBC2_RAM_SIZE, Mbc2};
pub(crate) use mbc3::Mbc3;
pub(crate) use mbc5::Mbc5;
pub(crate) use none::NoMbc;
pub(crate) use rtc::{RTC_SAVE_LEN, Rtc};

use super::cart_type::CartType;

pub(crate) const ROM_BANK_SIZE: usize = 0x4000;
pub(crate) const RAM_BANK_SIZE: usize = 0x2000;

/// Bank-controller backend owned by [`super::Cartridge`].
///
/// Extend only when a mapper is implemented and covered by tests — do not stub
/// exotic controllers as `None`.
#[derive(Debug, Clone)]
pub(crate) enum Mapper {
    None(NoMbc),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl Mapper {
    /// Build a mapper for a known supported cart type.
    ///
    /// Returns an error for unknown bytes or unimplemented controllers
    /// (e.g. `"unsupported mapper: MBC7"`).
    pub(crate) fn try_for_cart_type(
        cart_type: Option<CartType>,
        cart_type_byte: u8,
    ) -> Result<Self, String> {
        let Some(t) = cart_type else {
            return Err(format!("unsupported mapper: ${cart_type_byte:02X}"));
        };
        if t.is_rom_mapper() {
            return Ok(Self::None(NoMbc));
        }
        if t.is_mbc1() {
            return Ok(Self::Mbc1(Mbc1::new()));
        }
        if t.is_mbc2() {
            return Ok(Self::Mbc2(Mbc2::new()));
        }
        if t.is_mbc3() {
            return Ok(Self::Mbc3(Mbc3::new()));
        }
        if t.is_mbc5() {
            return Ok(Self::Mbc5(Mbc5::new()));
        }
        Err(format!("unsupported mapper: {}", t.name()))
    }

    pub(crate) fn switchable_rom_bank(&self) -> u16 {
        match self {
            Self::None(m) => u16::from(m.switchable_rom_bank()),
            Self::Mbc1(m) => u16::from(m.switchable_rom_bank()),
            Self::Mbc2(m) => u16::from(m.switchable_rom_bank()),
            Self::Mbc3(m) => u16::from(m.switchable_rom_bank()),
            Self::Mbc5(m) => m.switchable_rom_bank(),
        }
    }

    /// ROM bank mapped at `$0000–$3FFF` (MBC1 advanced mode may remap).
    pub(crate) fn fixed_rom_bank(&self) -> u16 {
        match self {
            Self::Mbc1(m) => u16::from(m.fixed_rom_bank()),
            _ => 0,
        }
    }

    /// Active external RAM bank when RAM access is allowed (not used for MBC2).
    pub(crate) fn active_ram_bank(&self) -> Option<u8> {
        match self {
            Self::None(m) => m.active_ram_bank(),
            Self::Mbc1(m) => m.active_ram_bank(),
            Self::Mbc2(_) => None,
            Self::Mbc3(m) => m.active_ram_bank(),
            Self::Mbc5(m) => m.active_ram_bank(),
        }
    }

    pub(crate) fn write_reg(&mut self, addr: u16, value: u8) {
        match self {
            Self::None(m) => m.write_reg(addr, value),
            Self::Mbc1(m) => m.write_reg(addr, value),
            Self::Mbc2(m) => m.write_reg(addr, value),
            Self::Mbc3(m) => m.write_reg(addr, value),
            Self::Mbc5(m) => m.write_reg(addr, value),
        }
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) {
        if let Self::Mbc3(m) = self {
            m.tick(t_cycles);
        }
    }

    pub(crate) fn mbc3_rtc_mut(&mut self) -> Option<&mut Rtc> {
        match self {
            Self::Mbc3(m) => Some(&mut m.rtc),
            _ => None,
        }
    }

    pub(crate) fn mbc3_read_rtc(&self) -> Option<u8> {
        match self {
            Self::Mbc3(m) => m.read_rtc(),
            _ => None,
        }
    }

    pub(crate) fn mbc3_write_rtc(&mut self, value: u8) -> bool {
        match self {
            Self::Mbc3(m) => m.write_rtc(value),
            _ => false,
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::MapperStateV1 {
        match self {
            Self::None(_) => crate::snapshot::MapperStateV1::None,
            Self::Mbc1(m) => crate::snapshot::MapperStateV1::Mbc1(m.snapshot()),
            Self::Mbc2(m) => crate::snapshot::MapperStateV1::Mbc2(m.snapshot()),
            Self::Mbc3(m) => crate::snapshot::MapperStateV1::Mbc3(m.snapshot()),
            Self::Mbc5(m) => crate::snapshot::MapperStateV1::Mbc5(m.snapshot()),
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::MapperStateV1) {
        match (self, state) {
            (Self::None(_), crate::snapshot::MapperStateV1::None) => {}
            (Self::Mbc1(m), crate::snapshot::MapperStateV1::Mbc1(s)) => m.apply(s),
            (Self::Mbc2(m), crate::snapshot::MapperStateV1::Mbc2(s)) => m.apply(s),
            (Self::Mbc3(m), crate::snapshot::MapperStateV1::Mbc3(s)) => m.apply(s),
            (Self::Mbc5(m), crate::snapshot::MapperStateV1::Mbc5(s)) => m.apply(s),
            _ => {}
        }
    }

    /// Reset banking registers to power-on defaults; MBC3 RTC clock contents are preserved.
    pub(crate) fn power_on_reset(&mut self) {
        match self {
            Self::None(_) => *self = Self::None(NoMbc),
            Self::Mbc1(m) => *m = Mbc1::new(),
            Self::Mbc2(m) => *m = Mbc2::new(),
            Self::Mbc5(m) => *m = Mbc5::new(),
            Self::Mbc3(m) => {
                let rtc = m.rtc.clone();
                *m = Mbc3::new();
                m.rtc = rtc;
            }
        }
    }
}

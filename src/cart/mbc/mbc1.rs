//! MBC1: ROM/RAM banking ([Pan Docs](https://gbdev.io/pandocs/MBC1.html)).
//!
//! Mode 0 (default): `$0000–$3FFF` fixed bank 0; `$4000–$7FFF` uses 5+2 bank bits.
//! Mode 1: advanced banking (upper bits also affect `$0000–$3FFF` / RAM).

#[derive(Debug, Clone)]
pub(crate) struct Mbc1 {
    ram_enabled: bool,
    /// Lower 5 bits of ROM bank (writes of `0` map to `1`).
    rom_bank_lo: u8,
    /// 2-bit RAM bank / ROM bank upper bits.
    bank_hi: u8,
    /// `false` = simple ROM banking; `true` = advanced.
    advanced_mode: bool,
}

impl Mbc1 {
    pub(crate) fn new() -> Self {
        Self {
            ram_enabled: false,
            rom_bank_lo: 1,
            bank_hi: 0,
            advanced_mode: false,
        }
    }

    pub(crate) fn switchable_rom_bank(&self) -> u8 {
        let lo = if self.rom_bank_lo == 0 {
            1
        } else {
            self.rom_bank_lo
        };
        if self.advanced_mode {
            lo
        } else {
            (self.bank_hi << 5) | lo
        }
    }

    /// ROM bank mapped at `$0000–$3FFF` (always 0 in simple mode).
    pub(crate) fn fixed_rom_bank(&self) -> u8 {
        if self.advanced_mode {
            self.bank_hi << 5
        } else {
            0
        }
    }

    pub(crate) fn active_ram_bank(&self) -> Option<u8> {
        if !self.ram_enabled {
            return None;
        }
        let bank = if self.advanced_mode { self.bank_hi } else { 0 };
        Some(bank)
    }

    pub(crate) fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank_lo = value & 0x1F;
            }
            0x4000..=0x5FFF => {
                self.bank_hi = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.advanced_mode = value & 0x01 != 0;
            }
            _ => {}
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::Mbc1StateV1 {
        crate::snapshot::Mbc1StateV1 {
            rom_bank: self.rom_bank_lo,
            ram_bank: self.bank_hi,
            ram_enable: self.ram_enabled,
            mode: u8::from(self.advanced_mode),
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::Mbc1StateV1) {
        self.rom_bank_lo = state.rom_bank;
        self.bank_hi = state.ram_bank;
        self.ram_enabled = state.ram_enable;
        self.advanced_mode = state.mode != 0;
    }
}

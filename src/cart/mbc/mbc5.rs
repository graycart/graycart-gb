//! Minimal MBC5: 9-bit ROM banking, RAM enable, RAM banking. No rumble.
//!
//! [Pan Docs — MBC5](https://gbdev.io/pandocs/MBC5.html)

#[derive(Debug, Clone)]
pub(crate) struct Mbc5 {
    ram_enabled: bool,
    /// Bank mapped at `$4000–$7FFF` (bits 0–8). Bank `0` is allowed.
    rom_bank: u16,
    /// RAM bank `$00–$0F`.
    ram_bank: u8,
}

impl Mbc5 {
    pub(crate) fn new() -> Self {
        Self {
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
        }
    }

    pub(crate) fn switchable_rom_bank(&self) -> u16 {
        self.rom_bank
    }

    pub(crate) fn active_ram_bank(&self) -> Option<u8> {
        if self.ram_enabled {
            Some(self.ram_bank)
        } else {
            None
        }
    }

    pub(crate) fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | u16::from(value);
            }
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0x0FF) | (u16::from(value & 0x01) << 8);
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x0F;
            }
            _ => {}
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::Mbc5StateV1 {
        crate::snapshot::Mbc5StateV1 {
            rom_bank_low: (self.rom_bank & 0xFF) as u8,
            rom_bank_high: ((self.rom_bank >> 8) & 0x01) as u8,
            ram_bank: self.ram_bank,
            ram_enable: self.ram_enabled,
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::Mbc5StateV1) {
        self.rom_bank = u16::from(state.rom_bank_low) | (u16::from(state.rom_bank_high) << 8);
        self.ram_bank = state.ram_bank;
        self.ram_enabled = state.ram_enable;
    }
}

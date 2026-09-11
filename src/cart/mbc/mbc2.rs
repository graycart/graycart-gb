//! MBC2: 4-bit ROM banking + 512×4-bit built-in RAM.
//!
//! [Pan Docs — MBC2](https://gbdev.io/pandocs/MBC2.html)

/// Built-in RAM: 512 half-bytes (saved as 512 bytes; high nibble unused).
pub(crate) const MBC2_RAM_SIZE: usize = 512;

#[derive(Debug, Clone)]
pub(crate) struct Mbc2 {
    ram_enabled: bool,
    /// ROM bank at `$4000–$7FFF` (lower 4 bits; `0` maps to `1`).
    rom_bank: u8,
}

impl Mbc2 {
    pub(crate) fn new() -> Self {
        Self {
            ram_enabled: false,
            rom_bank: 1,
        }
    }

    pub(crate) fn switchable_rom_bank(&self) -> u8 {
        let bank = self.rom_bank & 0x0F;
        if bank == 0 { 1 } else { bank }
    }

    /// Read built-in RAM (`$A000–$BFFF`); address uses only bits 0–8.
    /// Upper nibble of the returned byte is open-bus / undefined → read as `1`.
    pub(crate) fn read_ram(&self, addr: u16, ram: &[u8]) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        let idx = (addr as usize) & 0x1FF;
        let lo = ram.get(idx).copied().unwrap_or(0x0F) & 0x0F;
        lo | 0xF0
    }

    /// Write built-in RAM; only the low nibble is stored.
    /// Returns `true` if a byte was updated.
    pub(crate) fn write_ram(&self, addr: u16, value: u8, ram: &mut [u8]) -> bool {
        if !self.ram_enabled {
            return false;
        }
        let idx = (addr as usize) & 0x1FF;
        if let Some(slot) = ram.get_mut(idx) {
            *slot = value & 0x0F;
            true
        } else {
            false
        }
    }

    pub(crate) fn write_reg(&mut self, addr: u16, value: u8) {
        // Both RAM enable and ROM bank live in `$0000–$3FFF`; bit 8 of the
        // address selects which (Pan Docs).
        if !matches!(addr, 0x0000..=0x3FFF) {
            return;
        }
        if addr & 0x0100 == 0 {
            self.ram_enabled = value & 0x0F == 0x0A;
        } else {
            let bank = value & 0x0F;
            self.rom_bank = if bank == 0 { 1 } else { bank };
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::Mbc2StateV1 {
        crate::snapshot::Mbc2StateV1 {
            rom_bank: self.rom_bank,
            ram_enable: self.ram_enabled,
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::Mbc2StateV1) {
        self.rom_bank = state.rom_bank;
        self.ram_enabled = state.ram_enable;
    }
}

//! CGB WRAM banks + SVBK (`$FF70`). Echo RAM is not modeled here.
//!
//! `$C000–$CFFF` is always bank 0. `$D000–$DFFF` maps banks 1–7 via SVBK;
//! writing `0` selects bank 1. DMG (`cgb == false`) always uses bank 1 for `$D000`.

pub const SVBK: u16 = 0xFF70;

const BANK_BYTES: usize = 0x1000;

pub struct Wram {
    banks: [[u8; BANK_BYTES]; 8],
    /// 0–7; mapping 0 → bank 1 (canonicalized to 1 on write).
    svbk: u8,
}

impl Default for Wram {
    fn default() -> Self {
        Self::new()
    }
}

impl Wram {
    pub fn new() -> Self {
        Self {
            banks: [[0; BANK_BYTES]; 8],
            svbk: 1,
        }
    }

    pub fn mapped_bank_d(&self) -> usize {
        let bank = (self.svbk & 7) as usize;
        if bank == 0 { 1 } else { bank }
    }

    fn offset(addr: u16) -> usize {
        debug_assert!(
            (0xC000..0xE000).contains(&addr),
            "Wram only maps $C000-$DFFF, got {addr:#06X}"
        );
        (addr as usize) & 0x0FFF
    }

    fn bank_index(&self, addr: u16, cgb: bool) -> usize {
        if addr < 0xD000 {
            0
        } else if cgb {
            self.mapped_bank_d()
        } else {
            1
        }
    }

    pub fn read(&self, addr: u16, cgb: bool) -> u8 {
        let bank = self.bank_index(addr, cgb);
        self.banks[bank][Self::offset(addr)]
    }

    pub fn write(&mut self, addr: u16, value: u8, cgb: bool) {
        let bank = self.bank_index(addr, cgb);
        let offset = Self::offset(addr);
        self.banks[bank][offset] = value;
    }

    pub fn read_svbk(&self) -> u8 {
        0xF8 | (self.svbk & 7)
    }

    pub fn write_svbk(&mut self, value: u8) {
        let mut bank = value & 7;
        if bank == 0 {
            bank = 1;
        }
        self.svbk = bank;
    }

    pub fn snapshot_8k(&self, cgb: bool) -> [u8; 0x2000] {
        let mut out = [0u8; 0x2000];
        out[..BANK_BYTES].copy_from_slice(&self.banks[0]);
        let d = if cgb { self.mapped_bank_d() } else { 1 };
        out[BANK_BYTES..].copy_from_slice(&self.banks[d]);
        out
    }

    pub fn apply_snapshot_8k(&mut self, data: &[u8; 0x2000], cgb: bool) {
        self.banks[0].copy_from_slice(&data[..BANK_BYTES]);
        let d = if cgb { self.mapped_bank_d() } else { 1 };
        self.banks[d].copy_from_slice(&data[BANK_BYTES..]);
    }

    pub fn fill(&mut self, value: u8) {
        for bank in &mut self.banks {
            bank.fill(value);
        }
    }

    /// Zero all banks and map `$D000` to bank 1 (`SVBK` = 1).
    pub fn power_on_reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests;

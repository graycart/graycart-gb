//! CGB VRAM banks (`$8000`–`$9FFF`) and `VBK` (`$FF4F`).
//!
//! PPU owns both banks. CPU mapping uses VBK on CGB silicon; DMG always bank 0.

pub const VBK: u16 = 0xFF4F;

const BANK_SIZE: usize = 0x2000;

#[derive(Debug, Clone)]
pub struct Vram {
    banks: [[u8; BANK_SIZE]; 2],
    /// CPU bank select; only bit 0 is stored.
    vbk: u8,
}

impl Default for Vram {
    fn default() -> Self {
        Self::new()
    }
}

impl Vram {
    pub fn new() -> Self {
        Self {
            banks: [[0; BANK_SIZE]; 2],
            vbk: 0,
        }
    }

    pub fn cpu_read(&self, addr: u16, cgb: bool) -> u8 {
        self.banks[self.mapped_bank(cgb)][offset(addr)]
    }

    pub fn cpu_write(&mut self, addr: u16, value: u8, cgb: bool) {
        let bank = self.mapped_bank(cgb);
        self.banks[bank][offset(addr)] = value;
    }

    pub fn read_vbk(&self) -> u8 {
        0xFE | self.vbk
    }

    pub fn write_vbk(&mut self, value: u8) {
        self.vbk = value & 1;
    }

    pub fn bank(&self, i: usize) -> &[u8; BANK_SIZE] {
        debug_assert!(i < 2);
        &self.banks[i.min(1)]
    }

    pub fn bank0(&self) -> &[u8; BANK_SIZE] {
        self.bank(0)
    }

    pub fn bank0_mut(&mut self) -> &mut [u8; BANK_SIZE] {
        &mut self.banks[0]
    }

    pub fn fill(&mut self, value: u8) {
        for bank in &mut self.banks {
            bank.fill(value);
        }
    }

    fn mapped_bank(&self, cgb: bool) -> usize {
        if cgb { usize::from(self.vbk) } else { 0 }
    }
}

fn offset(addr: u16) -> usize {
    usize::from(addr) & (BANK_SIZE - 1)
}

#[cfg(test)]
mod tests;

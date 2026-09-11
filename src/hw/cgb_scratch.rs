//! CGB undocumented scratch `$FF72–$FF75`.
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html): `$FF72`/`$FF73`
//! fully R/W (init `$00`); `$FF74` fully R/W in CGB Mode else locked `$FF`;
//! `$FF75` bits 4–6 R/W (init 0), unused bits read as 1.

pub const FF72: u16 = 0xFF72;
pub const FF73: u16 = 0xFF73;
pub const FF74: u16 = 0xFF74;
pub const FF75: u16 = 0xFF75;

/// `$FF75` writable bits (4–6).
pub const FF75_RW: u8 = 0x70;
/// `$FF75` unused bits read as 1.
pub const FF75_UNUSED_ONES: u8 = 0x8F;

/// `$FF74` when not in CGB Mode (DMG silicon or DmgCompatibility).
pub fn ff74_locked() -> u8 {
    0xFF
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgbScratch {
    ff72: u8,
    ff73: u8,
    ff74: u8,
    ff75: u8,
}

impl Default for CgbScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl CgbScratch {
    pub fn new() -> Self {
        Self {
            ff72: 0,
            ff73: 0,
            ff74: 0,
            ff75: 0,
        }
    }

    pub fn read(self, addr: u16) -> Option<u8> {
        match addr {
            FF72 => Some(self.ff72),
            FF73 => Some(self.ff73),
            FF74 => Some(self.ff74),
            FF75 => Some((self.ff75 & FF75_RW) | FF75_UNUSED_ONES),
            _ => None,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        match addr {
            FF72 => self.ff72 = value,
            FF73 => self.ff73 = value,
            FF74 => self.ff74 = value,
            FF75 => self.ff75 = value & FF75_RW,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests;

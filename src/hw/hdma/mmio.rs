//! HDMA1–HDMA5 decode and address masks. No transfer state.
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html#lcd-vram-dma-transfers)

pub const HDMA1: u16 = 0xFF51;
pub const HDMA2: u16 = 0xFF52;
pub const HDMA3: u16 = 0xFF53;
pub const HDMA4: u16 = 0xFF54;
pub const HDMA5: u16 = 0xFF55;

/// Write to `$FF55`: length (`$10` blocks minus 1) and transfer mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hdma5Write {
    /// Low 7 bits of HDMA5: transfer length / `$10` − 1 (`$00`–`$7F`).
    pub blocks_minus_1: u8,
    /// Bit 7 set: HBlank DMA; clear: general-purpose DMA.
    pub hblank: bool,
}

/// Source address: lower 4 bits ignored (treated as 0).
pub fn mask_source(addr: u16) -> u16 {
    addr & 0xFFF0
}

/// Destination is always VRAM: bits 15–13 ignored, bits 3–0 ignored.
pub fn mask_dest(addr: u16) -> u16 {
    0x8000 | (addr & 0x1FF0)
}

/// After masking, source lies in `$8000`–`$9FFF` (copies garbage).
pub fn source_is_vram(addr: u16) -> bool {
    (0x8000..=0x9FFF).contains(&mask_source(addr))
}

pub fn parse_hdma5_write(value: u8) -> Hdma5Write {
    Hdma5Write {
        blocks_minus_1: value & 0x7F,
        hblank: value & 0x80 != 0,
    }
}

/// Encode `$FF55` read.
///
/// - Active: bit 7 = 0, low 7 = remaining / `$10` − 1.
/// - Complete / idle: `remaining_minus_1 == 0xFF` and not active → `$FF`.
/// - Abort (not active): bit 7 = 1, low 7 = remaining / `$10` − 1.
pub fn read_hdma5(remaining_minus_1: u8, active: bool) -> u8 {
    if active {
        remaining_minus_1 & 0x7F
    } else if remaining_minus_1 == 0xFF {
        0xFF
    } else {
        0x80 | (remaining_minus_1 & 0x7F)
    }
}

#[cfg(test)]
mod tests;

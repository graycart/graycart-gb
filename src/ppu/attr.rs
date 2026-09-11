//! CGB background/window tile-map attributes (VRAM bank 1).
//!
//! Same offset as the tile map byte. Bit 4 is unused.

use crate::ppu::framebuffer::Shade;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BgAttr {
    pub priority: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    pub tile_bank: u8, // 0 or 1
    pub palette: u8,   // 0–7
}

impl BgAttr {
    pub fn from_byte(b: u8) -> Self {
        Self {
            priority: b & 0x80 != 0,
            y_flip: b & 0x40 != 0,
            x_flip: b & 0x20 != 0,
            tile_bank: (b >> 3) & 1,
            palette: b & 0x07,
        }
    }

    pub fn fine_x(self, fine_x: usize) -> usize {
        let x = fine_x & 7;
        if self.x_flip { 7 - x } else { x }
    }

    pub fn fine_y(self, fine_y: u8) -> u8 {
        let y = fine_y & 7;
        if self.y_flip { 7 - y } else { y }
    }
}

/// RGB555 (bit 15 ignored). Sum R+G+B (0–93) → Shade quartiles for the DMG framebuffer stand-in.
pub fn shade_from_rgb555(color: u16) -> Shade {
    let r = color & 0x1F;
    let g = (color >> 5) & 0x1F;
    let b = (color >> 10) & 0x1F;
    let sum = r + g + b;
    match (sum * 4) / 94 {
        0 => Shade::Darkest,
        1 => Shade::Dark,
        2 => Shade::Light,
        _ => Shade::Lightest,
    }
}

#[cfg(test)]
mod tests;

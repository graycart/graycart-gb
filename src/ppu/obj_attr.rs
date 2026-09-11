//! CGB OAM attribute byte (OAM byte 3).
//!
//! Pan Docs OAM: bit 7 BG-over-OBJ, bit 6 Y flip, bit 5 X flip,
//! bit 4 Non-CGB OBP0/1 only (ignored here), bit 3 VRAM bank,
//! bits 2–0 OBJ palette 0–7.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjAttr {
    /// Bit 7: BG/Window color indices 1–3 are drawn over this OBJ.
    pub priority_bg_over_obj: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    pub tile_bank: u8, // 0 or 1
    pub palette: u8,   // 0–7
}

impl ObjAttr {
    pub fn from_byte(b: u8) -> Self {
        Self {
            priority_bg_over_obj: b & 0x80 != 0,
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

#[cfg(test)]
mod tests;

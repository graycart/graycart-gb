//! Object Attribute Memory (`$FE00`–`$FE9F`) — 40 sprites × 4 bytes.

pub const OAM_BASE: u16 = 0xFE00;
pub const OAM_SIZE: usize = 0xA0;
pub const SPRITE_COUNT: usize = 40;
pub const SPRITE_BYTES: usize = 4;
/// Max sprites considered on one scanline (hardware limit).
pub const SPRITES_PER_LINE: usize = 10;

/// One OAM entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    /// Byte 0: Y + 16 (screen Y = y − 16).
    pub y: u8,
    /// Byte 1: X + 8 (screen X = x − 8).
    pub x: u8,
    pub tile: u8,
    pub attrs: u8,
}

impl Sprite {
    pub fn from_bytes(b: [u8; 4]) -> Self {
        Self {
            y: b[0],
            x: b[1],
            tile: b[2],
            attrs: b[3],
        }
    }

    pub fn priority_behind_bg(self) -> bool {
        self.attrs & 0x80 != 0
    }

    pub fn flip_y(self) -> bool {
        self.attrs & 0x40 != 0
    }

    pub fn flip_x(self) -> bool {
        self.attrs & 0x20 != 0
    }

    pub fn obp1(self) -> bool {
        self.attrs & 0x10 != 0
    }

    /// Whether this sprite intersects scanline `ly` for the given height (8 or 16).
    pub fn intersects_line(self, ly: u8, height: u8) -> bool {
        let ly16 = u16::from(ly).wrapping_add(16);
        let y = u16::from(self.y);
        ly16 >= y && ly16 < y + u16::from(height)
    }

    /// Row within the sprite (0..height), accounting for Y flip.
    pub fn row_in_sprite(self, ly: u8, height: u8) -> u8 {
        let raw = ly.wrapping_add(16).wrapping_sub(self.y);
        if self.flip_y() {
            height.wrapping_sub(1).wrapping_sub(raw)
        } else {
            raw
        }
    }
}

/// 160-byte OAM mirror.
#[derive(Debug, Clone)]
pub struct Oam {
    data: [u8; OAM_SIZE],
}

impl Default for Oam {
    fn default() -> Self {
        Self::new()
    }
}

impl Oam {
    pub fn new() -> Self {
        Self {
            data: [0; OAM_SIZE],
        }
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        let off = addr.checked_sub(OAM_BASE)?;
        self.data.get(off as usize).copied()
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        let Some(off) = addr.checked_sub(OAM_BASE) else {
            return false;
        };
        if let Some(slot) = self.data.get_mut(off as usize) {
            *slot = value;
            true
        } else {
            false
        }
    }

    pub fn write_index(&mut self, index: usize, value: u8) {
        if let Some(slot) = self.data.get_mut(index) {
            *slot = value;
        }
    }

    pub fn sprite(&self, index: usize) -> Option<Sprite> {
        let base = index.checked_mul(SPRITE_BYTES)?;
        let b = [
            *self.data.get(base)?,
            *self.data.get(base + 1)?,
            *self.data.get(base + 2)?,
            *self.data.get(base + 3)?,
        ];
        Some(Sprite::from_bytes(b))
    }

    pub fn bytes(&self) -> &[u8; OAM_SIZE] {
        &self.data
    }
}

#[cfg(test)]
mod tests;

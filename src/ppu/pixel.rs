//! CGB composed-later pixel metadata (color id, BG attr.7, RGB555).
//!
//! Stores fields for Wave 2 BG/OBJ mix. Does **not** pick a winner.
//!
//! BG attr.7 ([Pan Docs Tile Maps](https://gbdev.io/pandocs/Tile_Maps.html)):
//! when set, color indices 1–3 of that BG/Window tile draw over OBJ.

use crate::ppu::attr::BgAttr;

/// Which layer produced this pixel. Wave 2 mixes BG and OBJ using these tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelSource {
    Bg,
    Obj,
}

/// One scanline pixel before BG/OBJ arbitration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CgbPixel {
    /// Tile/OBJ color index 0–3.
    pub color_id: u8,
    /// BG map attr.7 for BG pixels; `false` means no BG priority (OBJ pixels).
    pub bg_priority: bool,
    /// CRAM RGB555 (bit 15 unused).
    pub rgb555: u16,
    pub source: PixelSource,
}

impl CgbPixel {
    pub fn new(color_id: u8, bg_priority: bool, rgb555: u16, source: PixelSource) -> Self {
        Self {
            color_id: color_id & 0b11,
            bg_priority,
            rgb555,
            source,
        }
    }

    /// BG/window pixel: `bg_priority` from [`BgAttr::priority`] (attr.7).
    pub fn from_bg(attr: BgAttr, color_id: u8, rgb555: u16) -> Self {
        Self::new(color_id, attr.priority, rgb555, PixelSource::Bg)
    }

    /// OBJ pixel: `bg_priority` is always false (no BG attr.7).
    pub fn from_obj(color_id: u8, rgb555: u16) -> Self {
        Self::new(color_id, false, rgb555, PixelSource::Obj)
    }

    /// Color index 0 (OBJ transparent / BG color that loses to OBJ). Metadata only.
    pub fn is_transparent(self) -> bool {
        self.color_id == 0
    }
}

impl Default for CgbPixel {
    fn default() -> Self {
        Self::new(0, false, 0, PixelSource::Bg)
    }
}

#[cfg(test)]
mod tests;

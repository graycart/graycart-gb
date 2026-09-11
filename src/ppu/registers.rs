//! PPU I/O registers (LCDC, STAT selects, scroll, BGP, LYC).

/// `FF40` LCDC — LCD control.
pub const LCDC: u16 = 0xFF40;
/// `FF41` STAT — LCD status.
pub const STAT: u16 = 0xFF41;
/// `FF42` SCY — background viewport Y.
pub const SCY: u16 = 0xFF42;
/// `FF43` SCX — background viewport X.
pub const SCX: u16 = 0xFF43;
/// `FF44` LY — current scanline (0–153).
pub const LY: u16 = 0xFF44;
/// `FF45` LYC — LY compare.
pub const LYC: u16 = 0xFF45;
/// `FF47` BGP — background palette.
pub const BGP: u16 = 0xFF47;

pub const LCDC_ENABLE: u8 = 0x80;
pub const LCDC_WINDOW_TILE_MAP: u8 = 0x40; // 0 = $9800, 1 = $9C00
pub const LCDC_WINDOW_ENABLE: u8 = 0x20;
pub const LCDC_TILE_DATA: u8 = 0x10; // 1 = $8000 unsigned, 0 = $8800 signed
pub const LCDC_BG_TILE_MAP: u8 = 0x08; // 0 = $9800, 1 = $9C00
pub const LCDC_OBJ_SIZE: u8 = 0x04; // 0 = 8×8, 1 = 8×16
pub const LCDC_OBJ_ENABLE: u8 = 0x02;
pub const LCDC_BG_ENABLE: u8 = 0x01;
pub const STAT_WRITE_MASK: u8 = 0x78;

/// `FF46` DMA — high byte of source address (handled via [`crate::ppu::dma`]).
pub const DMA: u16 = 0xFF46;
/// `FF48` OBP0 — object palette 0.
pub const OBP0: u16 = 0xFF48;
/// `FF49` OBP1 — object palette 1.
pub const OBP1: u16 = 0xFF49;
/// `FF4A` WY — window Y position.
pub const WY: u16 = 0xFF4A;
/// `FF4B` WX — window X position + 7.
pub const WX: u16 = 0xFF4B;

/// Hardware power-on LCDC (`$00`, LCD off). Post-boot `$91` is [`LCDC_AFTER_BOOT`].
pub const LCDC_POWER_ON: u8 = 0x00;
/// DMG LCDC after boot ROM handoff (`$91`).
pub const LCDC_AFTER_BOOT: u8 = 0x91;
/// Common DMG BGP after boot (`$FC`).
pub const BGP_AFTER_BOOT: u8 = 0xFC;

#[derive(Debug, Clone)]
pub struct Registers {
    pub lcdc: u8,
    /// Writable STAT interrupt selects (bits 6–3).
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        Self {
            lcdc: LCDC_POWER_ON,
            stat: 0,
            scy: 0,
            scx: 0,
            lyc: 0,
            bgp: BGP_AFTER_BOOT,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
        }
    }

    pub fn lcd_enabled(&self) -> bool {
        self.lcdc & LCDC_ENABLE != 0
    }

    pub fn write_stat_selects(&mut self, value: u8) {
        self.stat = value & STAT_WRITE_MASK;
    }
}

#[cfg(test)]
mod tests;

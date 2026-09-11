//! CPU access rules for VRAM / OAM while the LCD is drawing.
//!
//! Pan Docs: Mode 2 locks OAM; Mode 3 locks OAM + VRAM; Modes 0/1 allow both.
//! With LCD off, both regions are always CPU-accessible.

use super::timing::PpuMode;

/// Whether the CPU may read/write `$FE00`–`$FE9F`.
pub fn cpu_can_access_oam(lcd_enabled: bool, mode: PpuMode) -> bool {
    if !lcd_enabled {
        return true;
    }
    matches!(mode, PpuMode::HBlank | PpuMode::VBlank)
}

/// Whether the CPU may read/write `$8000`–`$9FFF`.
pub fn cpu_can_access_vram(lcd_enabled: bool, mode: PpuMode) -> bool {
    if !lcd_enabled {
        return true;
    }
    matches!(mode, PpuMode::HBlank | PpuMode::VBlank | PpuMode::OamScan)
}

#[cfg(test)]
mod tests;

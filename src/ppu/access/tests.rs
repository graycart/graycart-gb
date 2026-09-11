use super::*;
use crate::ppu::PpuMode;

#[test]
fn lcd_off_allows_oam_and_vram_in_any_mode() {
    for mode in [
        PpuMode::HBlank,
        PpuMode::VBlank,
        PpuMode::OamScan,
        PpuMode::PixelTransfer,
    ] {
        assert!(cpu_can_access_oam(false, mode));
        assert!(cpu_can_access_vram(false, mode));
    }
}

#[test]
fn mode0_and_mode1_allow_oam_and_vram() {
    assert!(cpu_can_access_oam(true, PpuMode::HBlank));
    assert!(cpu_can_access_vram(true, PpuMode::HBlank));
    assert!(cpu_can_access_oam(true, PpuMode::VBlank));
    assert!(cpu_can_access_vram(true, PpuMode::VBlank));
}

#[test]
fn mode2_blocks_oam_allows_vram() {
    assert!(!cpu_can_access_oam(true, PpuMode::OamScan));
    assert!(cpu_can_access_vram(true, PpuMode::OamScan));
}

#[test]
fn mode3_blocks_oam_and_vram() {
    assert!(!cpu_can_access_oam(true, PpuMode::PixelTransfer));
    assert!(!cpu_can_access_vram(true, PpuMode::PixelTransfer));
}

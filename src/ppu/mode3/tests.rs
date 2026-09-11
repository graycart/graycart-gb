use super::*;
use crate::ppu::oam::Oam;
use crate::ppu::registers::Registers;

fn regs_with(scx: u8, wx: u8, wy: u8, lcdc: u8) -> Registers {
    let mut r = Registers::new();
    r.scx = scx;
    r.wx = wx;
    r.wy = wy;
    r.lcdc = lcdc;
    r
}

#[test]
fn base_length_without_penalties() {
    let regs = regs_with(0, 0xFF, 0xFF, 0x91); // LCD + BG, window off (bit5 clear)
    let oam = Oam::new();
    assert_eq!(mode3_length(0, &regs, &oam, false), MODE3_BASE);
}

#[test]
fn scx_fine_scroll_adds_zero_to_seven() {
    let oam = Oam::new();
    for fine in 0u8..8 {
        let regs = regs_with(fine, 0xFF, 0xFF, 0x91);
        assert_eq!(
            mode3_length(0, &regs, &oam, false),
            MODE3_BASE + u32::from(fine),
            "scx={fine}"
        );
    }
    let regs = regs_with(0x0F, 0xFF, 0xFF, 0x91); // % 8 == 7
    assert_eq!(mode3_length(0, &regs, &oam, false), MODE3_BASE + 7);
}

#[test]
fn window_adds_six_when_active_on_line() {
    // LCD + window + BG
    let regs = regs_with(0, 7, 0, 0xE1);
    let oam = Oam::new();
    assert_eq!(mode3_length(0, &regs, &oam, true), MODE3_BASE + 6);
    // Not yet active and WY doesn't match.
    let regs = regs_with(0, 7, 10, 0xE1);
    assert_eq!(mode3_length(0, &regs, &oam, false), MODE3_BASE);
}

#[test]
fn sprite_x_zero_adds_eleven() {
    let regs = regs_with(0, 0xFF, 0xFF, 0x93); // LCD + OBJ + BG
    let mut oam = Oam::new();
    oam.write(0xFE00, 16); // Y → line 0
    oam.write(0xFE01, 0); // X = 0 → fixed 11-dot penalty
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0);
    assert_eq!(mode3_length(0, &regs, &oam, false), MODE3_BASE + 11);
}

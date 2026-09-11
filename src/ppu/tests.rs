use super::*;
use registers::LCDC_ENABLE;

/// Tests that model a running LCD must opt in; power-on LCDC is off.
fn enable_lcd(ppu: &mut Ppu) {
    ppu.write(LCDC, LCDC_AFTER_BOOT);
}

#[test]
fn mode_bits_follow_line_cycles_past_mode2() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(68, 48, PpuMode::OamScan);
    ppu.tick(64); // 48 → 112
    assert_eq!(ppu.line_cycles(), 112);
    assert_eq!(ppu.mode(), PpuMode::PixelTransfer);
    assert_eq!(ppu.read(STAT).unwrap() & 0x03, 3);
}

/// Advance until Mode 0 on the current line (or LY changes).
fn tick_into_hblank(ppu: &mut Ppu) {
    let ly = ppu.ly();
    for _ in 0..CYCLES_PER_LINE {
        if ppu.ly() != ly || ppu.mode() == PpuMode::HBlank {
            break;
        }
        ppu.tick(1);
    }
    assert_eq!(ppu.ly(), ly, "expected to reach HBlank on LY={ly}");
    assert_eq!(ppu.mode(), PpuMode::HBlank);
}

#[test]
fn ly_increments_every_456_cycles() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    assert_eq!(ppu.ly(), 0);
    assert!(!ppu.tick(455).vblank);
    assert_eq!(ppu.ly(), 0);
    assert!(!ppu.tick(1).vblank);
    assert_eq!(ppu.ly(), 1);
}

#[test]
fn entering_ly_144_requests_vblank() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(143, CYCLES_PER_LINE - 1, PpuMode::HBlank);
    let irqs = ppu.tick(1);
    assert!(irqs.vblank);
    assert_eq!(ppu.ly(), 144);
    assert_eq!(ppu.mode(), PpuMode::VBlank);
}

#[test]
fn ly_wraps_after_153() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(153, CYCLES_PER_LINE - 1, PpuMode::VBlank);
    assert!(!ppu.tick(1).vblank);
    assert_eq!(ppu.ly(), 0);
    assert_eq!(ppu.mode(), PpuMode::OamScan);
}

#[test]
fn visible_line_mode_sequence_2_3_0() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    assert_eq!(ppu.mode(), PpuMode::OamScan);
    ppu.tick(MODE2_DOTS);
    assert_eq!(ppu.mode(), PpuMode::PixelTransfer);
    ppu.tick(MODE3_DOTS);
    assert_eq!(ppu.mode(), PpuMode::HBlank);
    assert_eq!(ppu.read(STAT).unwrap() & 0x03, 0);
}

#[test]
fn lcd_disable_resets_ly_and_freezes_timing() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(100, 200, PpuMode::PixelTransfer);
    ppu.framebuffer.set_pixel(0, 0, Shade::Darkest);
    ppu.write(LCDC, 0x00);
    assert!(!ppu.lcd_enabled());
    assert_eq!(ppu.ly(), 0);
    assert_eq!(ppu.line_cycles(), 0);
    assert_eq!(ppu.mode(), PpuMode::HBlank);
    assert_eq!(ppu.framebuffer.pixel(0, 0), Shade::Lightest);
    assert!(!ppu.tick(CYCLES_PER_LINE * 10).vblank);
    assert_eq!(ppu.ly(), 0);
}

#[test]
fn lcd_enable_resumes_scanlines() {
    let mut ppu = Ppu::new();
    ppu.write(LCDC, 0x00);
    ppu.write(LCDC, LCDC_ENABLE);
    assert!(ppu.lcd_enabled());
    assert_eq!(ppu.mode(), PpuMode::OamScan);
    ppu.tick(CYCLES_PER_LINE);
    assert_eq!(ppu.ly(), 1);
}

#[test]
fn no_vblank_while_lcd_disabled() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(143, CYCLES_PER_LINE - 1, PpuMode::HBlank);
    ppu.write(LCDC, 0x11);
    assert!(!ppu.tick(1).vblank);
    assert_eq!(ppu.ly(), 0);
}

#[test]
fn lyc_coincidence_flag_in_stat() {
    let mut ppu = Ppu::new();
    ppu.write(LYC, 0);
    assert_eq!(ppu.read(STAT).unwrap() & 0x04, 0x04);
    ppu.write(LYC, 5);
    assert_eq!(ppu.read(STAT).unwrap() & 0x04, 0);
    ppu.timing.ly = 5;
    assert_eq!(ppu.read(STAT).unwrap() & 0x04, 0x04);
}

#[test]
fn mode2_stat_interrupt_on_next_line() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    // Mid HBlank on LY=10 with Mode 2 STAT armed.
    ppu.set_line_state(10, MODE0_START, PpuMode::HBlank);
    ppu.write(STAT, 0x20);
    ppu.timing.sync_stat_from_regs(&ppu.regs);
    assert!(!ppu.timing.stat_line);

    let irqs = ppu.tick(CYCLES_PER_LINE - MODE0_START);
    assert_eq!(ppu.ly(), 11);
    assert_eq!(ppu.mode(), PpuMode::OamScan);
    assert!(
        irqs.stat,
        "Mode 2 STAT should rise at the start of the next line"
    );
}

#[test]
fn mode2_stat_rises_in_final_m_cycle_of_previous_line() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(10, MODE2_STAT_EARLY - 1, PpuMode::HBlank);
    ppu.write(STAT, 0x20);
    ppu.timing.sync_stat_from_regs(&ppu.regs);
    assert!(!ppu.timing.stat_line);

    let irqs = ppu.tick(1);
    assert_eq!(ppu.ly(), 10, "LY must not have wrapped yet");
    assert_eq!(ppu.mode(), PpuMode::HBlank);
    assert!(
        irqs.stat,
        "Mode 2 STAT source rises in the final M-cycle before the next OAM scan"
    );
}

/// Gekkio `vblank_stat_intr-C`: CGB Mode 2 STAT also at LY=144, one M-cycle
/// (4 T) before VBlank IF. DMG does not raise Mode 2 STAT for that wrap.
#[test]
fn dmg_mode2_stat_does_not_fire_entering_vblank() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(143, MODE2_STAT_EARLY - 1, PpuMode::HBlank);
    ppu.write(STAT, 0x20);
    ppu.timing.sync_stat_from_regs(&ppu.regs);
    assert!(!ppu.timing.stat_line);

    let early = ppu.tick(1);
    assert_eq!(ppu.ly(), 143);
    assert!(!early.vblank);
    assert!(
        !early.stat,
        "DMG must not raise Mode 2 STAT on the last M-cycle of LY=143"
    );

    let wrap = ppu.tick(4);
    assert_eq!(ppu.ly(), 144);
    assert!(wrap.vblank);
    assert!(!wrap.stat, "DMG VBlank wrap is not a Mode 2 STAT source");
}

#[test]
fn cgb_mode2_stat_rises_one_m_cycle_before_vblank() {
    let mut ppu = Ppu::new();
    ppu.set_cgb_silicon(true);
    enable_lcd(&mut ppu);
    ppu.set_line_state(143, MODE2_STAT_EARLY - 1, PpuMode::HBlank);
    ppu.write(STAT, 0x20);
    ppu.timing.sync_stat_from_regs(&ppu.regs);
    assert!(!ppu.timing.stat_line);

    let early = ppu.tick(1);
    assert_eq!(ppu.ly(), 143, "LY still 143 at Mode 2 STAT early window");
    assert!(
        !early.vblank,
        "VBlank IF must not coincide with the STAT edge"
    );
    assert!(
        early.stat,
        "CGB Mode 2 STAT source rises at dot 452 of LY=143"
    );

    let wrap = ppu.tick(4);
    assert_eq!(ppu.ly(), 144);
    assert!(wrap.vblank, "VBlank IF at the 143→144 wrap, 4 T later");
    assert!(
        !wrap.stat,
        "Mode 2 STAT already rose; wrap is VBlank IF only (STAT bit 5, no Mode 1 select)"
    );
}

#[test]
fn cgb_mode2_stat_still_rises_on_visible_lines() {
    let mut ppu = Ppu::new();
    ppu.set_cgb_silicon(true);
    enable_lcd(&mut ppu);
    ppu.set_line_state(10, MODE2_STAT_EARLY - 1, PpuMode::HBlank);
    ppu.write(STAT, 0x20);
    ppu.timing.sync_stat_from_regs(&ppu.regs);

    let irqs = ppu.tick(1);
    assert_eq!(ppu.ly(), 10);
    assert!(!irqs.vblank);
    assert!(irqs.stat);
}

#[test]
fn oam_unlocks_when_entering_hblank() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.set_line_state(0, 300, PpuMode::HBlank);
    ppu.write(0xFE00, 0x00);
    ppu.set_line_state(0, 0, PpuMode::OamScan);
    assert_eq!(ppu.read(0xFE00), Some(0xFF));
    ppu.tick(MODE0_START);
    assert_eq!(ppu.mode(), PpuMode::HBlank);
    assert_eq!(ppu.read(0xFE00), Some(0x00));
}

#[test]
fn lyc_stat_interrupt_when_ly_matches() {
    let mut ppu = Ppu::new();
    enable_lcd(&mut ppu);
    ppu.write(LYC, 1);
    ppu.write(STAT, 0x40);
    ppu.timing.sync_stat_from_regs(&ppu.regs);
    assert!(!ppu.timing.stat_line);
    let irqs = ppu.tick(CYCLES_PER_LINE);
    assert_eq!(ppu.ly(), 1);
    assert!(irqs.stat);
    assert_eq!(ppu.read(STAT).unwrap() & 0x04, 0x04);
}

#[test]
fn tick_paints_bg_scanline_into_framebuffer() {
    let mut ppu = Ppu::new();
    ppu.vram.bank0_mut()[0] = 0x3C;
    ppu.vram.bank0_mut()[1] = 0x7E;
    ppu.write(LCDC, 0b1001_0001);
    ppu.write(BGP, 0b11_10_01_00);
    ppu.write(SCX, 0);
    ppu.write(SCY, 0);

    // Advance through Mode 2+3 → HBlank paints LY=0
    tick_into_hblank(&mut ppu);
    let expected = [0u8, 2, 3, 3, 3, 3, 2, 0];
    for (x, &color) in expected.iter().enumerate() {
        assert_eq!(
            ppu.framebuffer.pixel(x, 0),
            Shade::from_index(color),
            "x={x}"
        );
    }
}

#[test]
fn tick_composites_sprite_over_bg() {
    let mut ppu = Ppu::new();
    // Sprite tile 1: leftmost pixel color 3
    let tile1 = 0x10;
    ppu.vram.bank0_mut()[tile1] = 0x80;
    ppu.vram.bank0_mut()[tile1 + 1] = 0x80;
    ppu.write(LCDC, 0b1001_0011); // LCD + $8000 + OBJ + BG
    ppu.write(BGP, 0b11_10_01_00);
    ppu.write(OBP0, 0b11_10_01_00);
    // OAM is CPU-locked in Mode 2; seed sprites while accessible.
    ppu.set_line_state(0, 300, PpuMode::HBlank);
    ppu.write(0xFE00, 16); // Y
    ppu.write(0xFE01, 8); // X → screen 0
    ppu.write(0xFE02, 1); // tile 1
    ppu.write(0xFE03, 0);
    ppu.set_line_state(0, 0, PpuMode::OamScan);

    tick_into_hblank(&mut ppu);
    assert_eq!(ppu.framebuffer.pixel(0, 0), Shade::Darkest);
}

#[test]
fn window_y_latch_and_line_counter() {
    let mut ppu = Ppu::new();
    ppu.vram.bank0_mut()[0] = 0xFF;
    ppu.vram.bank0_mut()[1] = 0xFF;
    // LCDC: LCD + win map $9C00 + window + $8000 tiles + BG
    ppu.write(LCDC, 0b1111_0001);
    ppu.write(BGP, 0b11_10_01_00);
    ppu.write(WY, 2);
    ppu.write(WX, 7);

    // Paint LY=0,1 — WY latch not yet true
    tick_into_hblank(&mut ppu);
    assert_eq!(ppu.window_line(), 0);
    ppu.tick(CYCLES_PER_LINE - ppu.line_cycles()); // → LY=1 Mode 2
    tick_into_hblank(&mut ppu);
    assert_eq!(ppu.window_line(), 0);

    // LY=2: WY matches → window draws and counter advances
    ppu.tick(CYCLES_PER_LINE - ppu.line_cycles()); // → LY=2 Mode 2
    tick_into_hblank(&mut ppu);
    assert_eq!(ppu.window_line(), 1);
    assert_eq!(ppu.framebuffer.pixel(0, 2), Shade::Darkest);

    // Next visible window line
    ppu.tick(CYCLES_PER_LINE - ppu.line_cycles()); // → LY=3 Mode 2
    tick_into_hblank(&mut ppu);
    assert_eq!(ppu.window_line(), 2);
}

/// Tile row `0x80, 0x00`: color 1 on the leftmost pixel. Attr `0x20` is X-flip.
fn seed_xflip_bg_line(ppu: &mut Ppu) {
    ppu.vram.bank0_mut()[0] = 0x80;
    ppu.vram.bank0_mut()[1] = 0x00;
    ppu.vram.write_vbk(1);
    ppu.vram.cpu_write(0x9800, 0x20, true);
    ppu.vram.write_vbk(0);
    ppu.cram.write_bgpi(0x80);
    ppu.cram.write_bgpd(0xFF, true);
    ppu.cram.write_bgpd(0x7F, true);
    ppu.cram.write_bgpi(0x80 | 2);
    ppu.cram.write_bgpd(0x00, true);
    ppu.cram.write_bgpd(0x00, true);
    ppu.write(LCDC, 0b1001_0001);
    ppu.write(BGP, 0b11_10_01_00);
}

#[test]
fn native_cgb_tick_applies_attr_x_flip_unlike_dmg() {
    let mut dmg = Ppu::new();
    seed_xflip_bg_line(&mut dmg);
    tick_into_hblank(&mut dmg);

    let mut cgb = Ppu::new();
    cgb.set_cgb_bg_attrs(true);
    seed_xflip_bg_line(&mut cgb);
    tick_into_hblank(&mut cgb);

    assert_eq!(dmg.framebuffer.pixel(0, 0), Shade::from_index(1));
    assert_eq!(dmg.framebuffer.pixel(7, 0), Shade::from_index(0));
    assert!(!dmg.framebuffer.presents_cgb_color());
    assert!(cgb.framebuffer.presents_cgb_color());
    assert_eq!(cgb.framebuffer.pixel(0, 0), Shade::Lightest);
    assert_eq!(cgb.framebuffer.pixel(7, 0), Shade::Darkest);
    assert_eq!(cgb.framebuffer.rgb555_pixel(0, 0), RGB555_WHITE);
    assert_eq!(cgb.framebuffer.rgb555_pixel(7, 0), RGB555_BLACK);
}

const RGB555_WHITE: u16 = 0x7FFF;
const RGB555_BLACK: u16 = 0x0000;

fn write_bg_rgb555(ppu: &mut Ppu, palette: u8, color: u8, rgb: u16) {
    let idx = palette * 8 + color * 2;
    ppu.cram.write_bgpi(0x80 | idx);
    ppu.cram.write_bgpd(rgb as u8, true);
    ppu.cram.write_bgpd((rgb >> 8) as u8, true);
}

fn write_obj_rgb555(ppu: &mut Ppu, palette: u8, color: u8, rgb: u16) {
    let idx = palette * 8 + color * 2;
    ppu.cram.write_obpi(0x80 | idx);
    ppu.cram.write_obpd(rgb as u8, true);
    ppu.cram.write_obpd((rgb >> 8) as u8, true);
}

/// Solid color-3 tile 0. Color 0 is white so a blank DMG line stays Lightest.
fn seed_native_cgb_solid_bg_color3(ppu: &mut Ppu, lcdc: u8, bg_attr: u8, color3: u16) {
    ppu.vram.bank0_mut()[0] = 0xFF;
    ppu.vram.bank0_mut()[1] = 0xFF;
    ppu.vram.write_vbk(1);
    ppu.vram.cpu_write(0x9800, bg_attr, true);
    ppu.vram.write_vbk(0);
    write_bg_rgb555(ppu, 0, 0, RGB555_WHITE);
    write_bg_rgb555(ppu, 0, 3, color3);
    ppu.write(LCDC, lcdc);
    ppu.write(BGP, 0b11_10_01_00);
}

fn seed_opaque_obj_at_origin(ppu: &mut Ppu, tile: u8, oam_attr: u8) {
    let tile_off = usize::from(tile) * 16;
    ppu.vram.bank0_mut()[tile_off] = 0x80;
    ppu.vram.bank0_mut()[tile_off + 1] = 0x80;
    write_obj_rgb555(ppu, oam_attr & 0x07, 3, RGB555_BLACK);
    ppu.set_line_state(0, 300, PpuMode::HBlank);
    ppu.write(0xFE00, 16);
    ppu.write(0xFE01, 8);
    ppu.write(0xFE02, tile);
    ppu.write(0xFE03, oam_attr);
    ppu.set_line_state(0, 0, PpuMode::OamScan);
}

#[test]
fn native_cgb_tick_lcdc0_clear_still_draws_bg_unlike_dmg() {
    // LCD + $8000 tiles; LCDC.0 clear. DMG blanks to Lightest; NativeCgb still fetches the tile.
    let lcdc = 0b1001_0000;
    let mut dmg = Ppu::new();
    seed_native_cgb_solid_bg_color3(&mut dmg, lcdc, 0, RGB555_BLACK);
    tick_into_hblank(&mut dmg);

    let mut cgb = Ppu::new();
    cgb.set_cgb_bg_attrs(true);
    seed_native_cgb_solid_bg_color3(&mut cgb, lcdc, 0, RGB555_BLACK);
    tick_into_hblank(&mut cgb);

    assert_eq!(dmg.framebuffer.pixel(0, 0), Shade::Lightest);
    assert_ne!(
        cgb.framebuffer.pixel(0, 0),
        Shade::Lightest,
        "NativeCgb LCDC.0 clear must still draw BG (not DMG blank)"
    );
    assert_eq!(cgb.framebuffer.pixel(0, 0), shade_from_rgb555(RGB555_BLACK));
}

#[test]
fn native_cgb_tick_bg_color0_opaque_obj_uses_obj_cram_shade() {
    let mut ppu = Ppu::new();
    ppu.set_cgb_bg_attrs(true);
    write_bg_rgb555(&mut ppu, 0, 0, RGB555_WHITE);
    ppu.write(LCDC, 0b1001_0011);
    ppu.write(BGP, 0b11_10_01_00);
    seed_opaque_obj_at_origin(&mut ppu, 1, 0);

    tick_into_hblank(&mut ppu);
    assert_eq!(
        ppu.framebuffer.pixel(0, 0),
        shade_from_rgb555(RGB555_BLACK),
        "BG index 0 must lose to opaque OBJ CRAM shade"
    );
}

#[test]
fn native_cgb_tick_lcdc0_set_both_attr7_clear_obj_wins() {
    let mut ppu = Ppu::new();
    ppu.set_cgb_bg_attrs(true);
    seed_native_cgb_solid_bg_color3(&mut ppu, 0b1001_0011, 0, RGB555_WHITE);
    seed_opaque_obj_at_origin(&mut ppu, 1, 0);

    tick_into_hblank(&mut ppu);
    assert_eq!(
        ppu.framebuffer.pixel(0, 0),
        shade_from_rgb555(RGB555_BLACK),
        "LCDC.0 set and both attr.7 clear ⇒ OBJ over BG 1–3"
    );
}

#[test]
fn native_cgb_tick_lcdc0_set_bg_or_oam_attr7_bg_wins() {
    for (bg_attr, oam_attr) in [(0x80u8, 0u8), (0, 0x80)] {
        let mut ppu = Ppu::new();
        ppu.set_cgb_bg_attrs(true);
        seed_native_cgb_solid_bg_color3(&mut ppu, 0b1001_0011, bg_attr, RGB555_WHITE);
        seed_opaque_obj_at_origin(&mut ppu, 1, oam_attr);

        tick_into_hblank(&mut ppu);
        assert_eq!(
            ppu.framebuffer.pixel(0, 0),
            shade_from_rgb555(RGB555_WHITE),
            "LCDC.0 set and BG or OAM attr.7 ⇒ BG 1–3 over OBJ (bg_attr={bg_attr:#04x} oam={oam_attr:#04x})"
        );
    }
}

#[test]
fn native_cgb_tick_overlapping_obj_earlier_oam_wins_despite_higher_x() {
    let mut ppu = Ppu::new();
    ppu.set_cgb_bg_attrs(true);
    write_bg_rgb555(&mut ppu, 0, 0, RGB555_WHITE);
    write_obj_rgb555(&mut ppu, 0, 3, RGB555_BLACK);
    write_obj_rgb555(&mut ppu, 1, 3, RGB555_WHITE);
    // Tile 1 / 2: solid color 3.
    for tile in [1u8, 2] {
        let off = usize::from(tile) * 16;
        ppu.vram.bank0_mut()[off] = 0xFF;
        ppu.vram.bank0_mut()[off + 1] = 0xFF;
    }
    ppu.write(LCDC, 0b1001_0011);
    ppu.set_line_state(0, 300, PpuMode::HBlank);
    // OAM 0: X=20 (screen 12–19), pal 0 black. OAM 1: X=16 (screen 8–15), pal 1 white.
    ppu.write(0xFE00, 16);
    ppu.write(0xFE01, 20);
    ppu.write(0xFE02, 1);
    ppu.write(0xFE03, 0);
    ppu.write(0xFE04, 16);
    ppu.write(0xFE05, 16);
    ppu.write(0xFE06, 2);
    ppu.write(0xFE07, 1);
    ppu.set_line_state(0, 0, PpuMode::OamScan);

    tick_into_hblank(&mut ppu);
    assert_eq!(
        ppu.framebuffer.pixel(12, 0),
        shade_from_rgb555(RGB555_BLACK),
        "NativeCgb OBJ-vs-OBJ is OAM order: earlier sprite wins at overlap despite higher X"
    );
}

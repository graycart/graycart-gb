use super::*;
use crate::ppu::cram::Cram;
use crate::ppu::framebuffer::Shade;
use crate::ppu::tile::tile_row_offset;

const LCDC_OBJ_8X8: u8 = 0b1000_0010;

fn put_tile_row(vram: &mut [u8], tile_addr: u16, row: u8, lo: u8, hi: u8) {
    let off = (tile_addr - 0x8000) as usize + tile_row_offset(row);
    vram[off] = lo;
    vram[off + 1] = hi;
}

#[test]
fn sprite_draws_opaque_pixels() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0);

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0x80, 0x80);

    let lcdc = LCDC_OBJ_8X8; // LCD + OBJ 8x8
    let mut fb = Framebuffer::new();
    let bg = [0u8; SCREEN_WIDTH];
    render_scanline(
        &SpriteLine {
            ly: 0,
            lcdc,
            obp0: 0b11_10_01_00,
            obp1: 0,
            oam: &oam,
            vram: &vram,
            bg_colors: &bg,
            vram1: None,
            cram: None,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(fb.pixel(1, 0), Shade::Lightest);
}

#[test]
fn sprite_behind_bg_skips_nonzero_bg() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0x80); // behind BG

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF); // solid color 3

    let lcdc = LCDC_OBJ_8X8;
    let mut fb = Framebuffer::new();
    let mut bg = [0u8; SCREEN_WIDTH];
    bg[0] = 1;
    fb.set_pixel(0, 0, Shade::Light);
    render_scanline(
        &SpriteLine {
            ly: 0,
            lcdc,
            obp0: 0b11_10_01_00,
            obp1: 0,
            oam: &oam,
            vram: &vram,
            bg_colors: &bg,
            vram1: None,
            cram: None,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Light); // BG kept
    assert_eq!(fb.pixel(1, 0), Shade::Darkest); // over BG color 0
}

#[test]
fn ten_sprite_limit_uses_oam_order() {
    let mut oam = Oam::new();
    for i in 0..12u8 {
        let base = 0xFE00 + u16::from(i) * 4;
        oam.write(base, 16);
        oam.write(base + 1, 8 + i); // increasing X
        oam.write(base + 2, 0);
        oam.write(base + 3, 0);
    }
    let hit = sprites_for_line(&oam, 0, 8);
    assert_eq!(hit.len(), 10);
    assert_eq!(hit[0].0, 0);
    assert_eq!(hit[9].0, 9);
}

fn write_obj_rgb555(cram: &mut Cram, palette: u8, color: u8, rgb555: u16) {
    let addr = palette * 8 + color * 2;
    cram.write_obpi(0x80 | addr);
    cram.write_obpd((rgb555 & 0xFF) as u8, true);
    cram.write_obpd((rgb555 >> 8) as u8, true);
}

/// OAM0 at X=16 (screen 8), OAM1 at X=12 (screen 4) — overlap at x 8–11.
fn overlapping_oam(tile0: u8, tile1: u8, attr0: u8, attr1: u8) -> Oam {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 16);
    oam.write(0xFE02, tile0);
    oam.write(0xFE03, attr0);
    oam.write(0xFE04, 16);
    oam.write(0xFE05, 12);
    oam.write(0xFE06, tile1);
    oam.write(0xFE07, attr1);
    oam
}

fn sprite_line<'a>(
    ly: u8,
    oam: &'a Oam,
    vram: &'a [u8],
    vram1: Option<&'a [u8]>,
    cram: Option<&'a Cram>,
) -> SpriteLine<'a> {
    static BG: [u8; SCREEN_WIDTH] = [0u8; SCREEN_WIDTH];
    SpriteLine {
        ly,
        lcdc: LCDC_OBJ_8X8,
        obp0: 0b11_10_01_00,
        obp1: 0,
        oam,
        vram,
        bg_colors: &BG,
        vram1,
        cram,
    }
}

#[test]
fn cgb_overlapping_earlier_oam_wins_despite_higher_x() {
    let oam = overlapping_oam(0, 1, 0, 0);
    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF); // tile 0: color 3
    put_tile_row(&mut vram, 0x8010, 0, 0xFF, 0x00); // tile 1: color 1
    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 3, 0x001F);
    write_obj_rgb555(&mut cram, 0, 1, 0x03E0);

    let line = sprite_line(0, &oam, &vram, None, Some(&cram));
    let hit = sprites_for_line_mode(&oam, 0, 8, true);
    assert_eq!(hit[0].0, 0);
    assert_eq!(hit[1].0, 1);

    let samples = collect_cgb_obj_line(&line);
    let overlap = samples[8].expect("overlap pixel");
    assert_eq!(overlap.color_id, 3);
    assert_eq!(overlap.rgb555, 0x001F);
}

#[test]
fn dmg_overlapping_lower_x_wins() {
    let oam = overlapping_oam(0, 1, 0, 0);
    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF); // color 3 → Darkest
    put_tile_row(&mut vram, 0x8010, 0, 0xFF, 0x00); // color 1 → Light

    let hit = sprites_for_line(&oam, 0, 8);
    assert_eq!(hit[0].0, 1, "lower X (OAM1) sorts first on DMG");
    assert_eq!(hit[1].0, 0);

    let mut fb = Framebuffer::new();
    let bg = [0u8; SCREEN_WIDTH];
    render_scanline(
        &SpriteLine {
            ly: 0,
            lcdc: LCDC_OBJ_8X8,
            obp0: 0b11_10_01_00,
            obp1: 0,
            oam: &oam,
            vram: &vram,
            bg_colors: &bg,
            vram1: None,
            cram: None,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(8, 0), Shade::Light);
}

#[test]
fn cgb_obj_palette_1_uses_distinct_cram_rgb555() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0x01); // CGB palette 1; bit 4 unused

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0x80, 0x80); // leftmost color 3

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 3, 0x001F);
    write_obj_rgb555(&mut cram, 1, 3, 0x03E0);

    let line = sprite_line(0, &oam, &vram, None, Some(&cram));
    let sample = collect_cgb_obj_line(&line)[0].expect("opaque OBJ");
    assert_eq!(sample.color_id, 3);
    assert_eq!(sample.rgb555, 0x03E0);
    assert_ne!(sample.rgb555, cram.rgb555_obj(0, 3));
}

#[test]
fn cgb_obj_tile_from_vram_bank1() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0x08); // VRAM bank 1

    let vram = [0u8; 0x2000];
    let mut vram1 = [0u8; 0x2000];
    put_tile_row(&mut vram1, 0x8000, 0, 0x80, 0x00); // leftmost color 1

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 1, 0x7C00);

    let line = sprite_line(0, &oam, &vram, Some(&vram1), Some(&cram));
    let sample = collect_cgb_obj_line(&line)[0].expect("bank1 tile");
    assert_eq!(sample.color_id, 1);
    assert_eq!(sample.rgb555, 0x7C00);
}

#[test]
fn cgb_obj_color0_does_not_occupy_slot() {
    let mut oam = Oam::new();
    // Same X: earlier tile left 4 px color 1, right 4 transparent
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0);
    oam.write(0xFE04, 16);
    oam.write(0xFE05, 8);
    oam.write(0xFE06, 1);
    oam.write(0xFE07, 0);

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xF0, 0x00); // color 1 then 0s
    put_tile_row(&mut vram, 0x8010, 0, 0x00, 0xFF); // color 2 solid

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 1, 0x001F);
    write_obj_rgb555(&mut cram, 0, 2, 0x03E0);

    let line = sprite_line(0, &oam, &vram, None, Some(&cram));
    let samples = collect_cgb_obj_line(&line);
    let left = samples[0].expect("earlier opaque");
    assert_eq!(left.color_id, 1);
    assert_eq!(left.rgb555, 0x001F);
    let right = samples[4].expect("later through color 0");
    assert_eq!(right.color_id, 2);
    assert_eq!(right.rgb555, 0x03E0);
}

const LCDC_OBJ_8X16: u8 = 0b1000_0110; // LCD + OBJ + 8×16

#[test]
fn cgb_obj_attr7_sets_bg_over_and_does_not_drop_for_bg() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0x80); // OAM attr.7 BG-over-OBJ

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0x80, 0x80);

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 3, 0x7C00);

    let bg = [1u8; SCREEN_WIDTH];
    let line = SpriteLine {
        ly: 0,
        lcdc: LCDC_OBJ_8X8,
        obp0: 0,
        obp1: 0,
        oam: &oam,
        vram: &vram,
        bg_colors: &bg,
        vram1: None,
        cram: Some(&cram),
    };
    let sample = collect_cgb_obj_line(&line)[0].expect("OBJ still collected");
    assert!(sample.bg_over);
    assert_eq!(sample.color_id, 3);
    assert_eq!(sample.rgb555, 0x7C00);
}

#[test]
fn cgb_8x16_uses_bottom_tile_not_fine_y() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0); // even id; bottom tile is 1
    oam.write(0xFE03, 0);

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00); // tile 0 row 0: color 1
    put_tile_row(&mut vram, 0x8010, 0, 0x80, 0x80); // tile 1 row 0: color 3

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 1, 0x001F);
    write_obj_rgb555(&mut cram, 0, 3, 0x03E0);

    let mut line = sprite_line(0, &oam, &vram, None, Some(&cram));
    line.lcdc = LCDC_OBJ_8X16;
    let top = collect_cgb_obj_line(&line)[0].expect("top half");
    assert_eq!(top.color_id, 1);

    line.ly = 8;
    let bottom = collect_cgb_obj_line(&line)[0].expect("bottom half");
    assert_eq!(bottom.color_id, 3);
}

#[test]
fn cgb_8x16_y_flip_uses_full_height_row() {
    let mut oam = Oam::new();
    oam.write(0xFE00, 16);
    oam.write(0xFE01, 8);
    oam.write(0xFE02, 0);
    oam.write(0xFE03, 0x40); // Y flip

    let mut vram = [0u8; 0x2000];
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00); // tile 0 row 0: color 1
    put_tile_row(&mut vram, 0x8010, 7, 0x80, 0x80); // tile 1 row 7: color 3

    let mut cram = Cram::new();
    write_obj_rgb555(&mut cram, 0, 1, 0x001F);
    write_obj_rgb555(&mut cram, 0, 3, 0x03E0);

    let mut line = sprite_line(0, &oam, &vram, None, Some(&cram));
    line.lcdc = LCDC_OBJ_8X16;
    let sample = collect_cgb_obj_line(&line)[0].expect("Y-flipped top of screen");
    // row_in_sprite(0,16) with Y flip = 15 → tile 1, row 7
    assert_eq!(sample.color_id, 3);
    assert_eq!(sample.rgb555, 0x03E0);
}

use super::*;
use crate::ppu::framebuffer::Shade;

fn blank_vram() -> [u8; 0x2000] {
    [0; 0x2000]
}

fn put_tile_row(vram: &mut [u8], tile_addr: u16, row: u8, lo: u8, hi: u8) {
    let off = (tile_addr - 0x8000) as usize + tile_row_offset(row);
    vram[off] = lo;
    vram[off + 1] = hi;
}

#[test]
fn known_tile_row_fills_scanline_with_bgp_shades() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0x3C, 0x7E);
    let lcdc = 0b1001_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 0,
            scy: 0,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    let expected = [0, 2, 3, 3, 3, 3, 2, 0];
    for x in 0..SCREEN_WIDTH {
        let want = Shade::from_index(expected[x % 8]);
        assert_eq!(fb.pixel(x, 0), want, "x={x}");
    }
}

#[test]
fn scx_wraps_at_256() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram, 0x8010, 0, 0x00, 0xFF);
    vram[0x9800 - 0x8000] = 0;
    vram[(0x9800 - 0x8000) + 31] = 1;

    let lcdc = 0b1001_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 248,
            scy: 0,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Dark);
    assert_eq!(fb.pixel(8, 0), Shade::Light);
}

#[test]
fn scy_selects_tile_row_with_wrap() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram, 0x8000, 1, 0x00, 0xFF);
    let lcdc = 0b1001_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 0,
            scy: 0,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Light);

    fb.clear(Shade::Lightest);
    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 0,
            scy: 1,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Dark);

    put_tile_row(&mut vram, 0x8000, 7, 0xFF, 0xFF);
    vram[(0x9800 - 0x8000) + 31 * 32] = 0;
    fb.clear(Shade::Lightest);
    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 0,
            scy: 255,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
}

#[test]
fn bg_disabled_fills_lightest() {
    let vram = blank_vram();
    let mut fb = Framebuffer::new();
    fb.set_pixel(0, 0, Shade::Darkest);
    let mut colors = [0u8; SCREEN_WIDTH];
    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0x80,
            scx: 0,
            scy: 0,
            bgp: 0xE4,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
    assert_eq!(fb.pixel(159, 0), Shade::Lightest);
}

fn put_attr(vram1: &mut [u8], map_addr: u16, attr: u8) {
    vram1[(map_addr - 0x8000) as usize] = attr;
}

fn write_bg_rgb555(cram: &mut crate::ppu::cram::Cram, palette: u8, color: u8, rgb555: u16) {
    let addr = palette * 8 + color * 2;
    cram.write_bgpi(0x80 | addr);
    cram.write_bgpd((rgb555 & 0xFF) as u8, true);
    cram.write_bgpd((rgb555 >> 8) as u8, true);
}

#[test]
fn cgb_bank1_tile_is_used_when_attr_selects_bank() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram1, 0x8000, 0, 0x00, 0xFF);
    put_attr(&mut vram1, 0x9800, 0x08);
    let cram = crate::ppu::cram::Cram::new();
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0b1001_0001,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(colors[0], 2);
    assert_eq!(colors[7], 2);
}

#[test]
fn cgb_x_flip_reverses_tile_row() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0x80, 0x00);
    put_attr(&mut vram1, 0x9800, 0x20);
    let cram = crate::ppu::cram::Cram::new();
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0b1001_0001,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(&colors[0..8], &[0, 0, 0, 0, 0, 0, 0, 1]);
}

#[test]
fn cgb_y_flip_selects_opposite_row() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram, 0x8000, 7, 0xFF, 0xFF);
    put_attr(&mut vram1, 0x9800, 0x40);
    let cram = crate::ppu::cram::Cram::new();
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0b1001_0001,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(colors[0], 3);
}

#[test]
fn cgb_palette_1_cram_differs_from_palette_0() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    put_attr(&mut vram1, 0x9800, 0x00);
    put_attr(&mut vram1, 0x9801, 0x01);
    let mut cram = crate::ppu::cram::Cram::new();
    write_bg_rgb555(&mut cram, 0, 3, 0x0000);
    write_bg_rgb555(&mut cram, 1, 3, 0x7FFF);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0b1001_0001,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(colors[0], 3);
    assert_eq!(colors[8], 3);
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(fb.pixel(8, 0), Shade::Lightest);
}

#[test]
fn cgb_priority_bit_does_not_hide_background() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    put_attr(&mut vram1, 0x9800, 0x80);
    let mut cram = crate::ppu::cram::Cram::new();
    write_bg_rgb555(&mut cram, 0, 3, 0x0000);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc: 0b1001_0001,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(colors[0], 3);
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
}

#[test]
fn native_cgb_lcdc0_clear_still_draws_tile() {
    let mut vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    put_attr(&mut vram1, 0x9800, 0x80);
    let mut cram = crate::ppu::cram::Cram::new();
    write_bg_rgb555(&mut cram, 0, 3, 0x0000);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];
    let mut bg_priority = [false; SCREEN_WIDTH];
    let lcdc = 0b1001_0000; // LCD + $8000 tiles; LCDC.0 clear

    render_scanline(
        &mut BgLine {
            ly: 0,
            lcdc,
            scx: 0,
            scy: 0,
            bgp: 0b11_10_01_00,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: Some(&mut bg_priority),
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    );

    assert_eq!(colors[0], 3, "LCDC.0 must not blank Native CGB BG");
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert!(bg_priority[0], "attr.7 should fill bg_priority");
}

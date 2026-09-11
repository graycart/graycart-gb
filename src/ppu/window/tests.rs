use super::*;
use crate::ppu::cram::Cram;
use crate::ppu::framebuffer::Shade;
use crate::ppu::tile::tile_row_offset;

fn set_bg_rgb555(cram: &mut Cram, palette: u8, color: u8, rgb555: u16) {
    let offset = palette * 8 + color * 2;
    cram.write_bgpi(0x80 | offset);
    cram.write_bgpd((rgb555 & 0xFF) as u8, true);
    cram.write_bgpd((rgb555 >> 8) as u8, true);
}

fn blank_vram() -> [u8; 0x2000] {
    [0; 0x2000]
}

fn put_tile_row(vram: &mut [u8], tile_addr: u16, row: u8, lo: u8, hi: u8) {
    let off = (tile_addr - 0x8000) as usize + tile_row_offset(row);
    vram[off] = lo;
    vram[off + 1] = hi;
}

#[test]
fn window_overlays_from_wx_minus_7() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let lcdc = 0b1111_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];
    for x in 0..SCREEN_WIDTH {
        fb.set_pixel(x, 0, Shade::Lightest);
    }

    assert!(render_scanline(
        &mut WindowLine {
            ly: 0,
            lcdc,
            wx: 7,
            y_active: true,
            win_line: 0,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(fb.pixel(159, 0), Shade::Darkest);
    assert_eq!(colors[0], 3);
}

#[test]
fn window_starts_mid_scanline() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let lcdc = 0b1111_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];
    for x in 0..SCREEN_WIDTH {
        fb.set_pixel(x, 5, Shade::Light);
    }

    assert!(render_scanline(
        &mut WindowLine {
            ly: 5,
            lcdc,
            wx: 87,
            y_active: true,
            win_line: 0,
            bgp,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb
    ));
    assert_eq!(fb.pixel(79, 5), Shade::Light);
    assert_eq!(fb.pixel(80, 5), Shade::Darkest);
}

#[test]
fn disabled_or_wx_out_of_range_draws_nothing() {
    let vram = blank_vram();
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];
    fb.set_pixel(0, 0, Shade::Dark);
    assert!(!render_scanline(
        &mut WindowLine {
            ly: 0,
            lcdc: 0b1001_0001,
            wx: 7,
            y_active: true,
            win_line: 0,
            bgp: 0xE4,
            vram: &vram,
            vram1: None,
            cram: None,
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Dark);
    assert!(!window_visible_on_line(0b1111_0001, true, 167));
}

#[test]
fn win_line_selects_tile_row() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram, 0x8000, 1, 0x00, 0xFF);
    let lcdc = 0b1111_0001;
    let bgp = 0b11_10_01_00;
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    render_scanline(
        &mut WindowLine {
            ly: 0,
            lcdc,
            wx: 7,
            y_active: true,
            win_line: 0,
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
    render_scanline(
        &mut WindowLine {
            ly: 1,
            lcdc,
            wx: 7,
            y_active: true,
            win_line: 1,
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
    assert_eq!(fb.pixel(0, 1), Shade::Dark);
}

const WIN_MAP_ATTR: usize = 0x9C00 - 0x8000;
const LCDC_WIN_8000: u8 = 0b1111_0001;
const BGP_ID: u8 = 0b11_10_01_00;

fn render_cgb_window(
    vram: &[u8],
    vram1: &[u8],
    cram: &Cram,
    ly: u8,
    win_line: u8,
    fb: &mut Framebuffer,
    colors: &mut [u8; SCREEN_WIDTH],
) -> bool {
    render_scanline(
        &mut WindowLine {
            ly,
            lcdc: LCDC_WIN_8000,
            wx: 7,
            y_active: true,
            win_line,
            bgp: BGP_ID,
            vram,
            vram1: Some(vram1),
            cram: Some(cram),
            bg_priority: None,
            cgb: None,
            colors,
        },
        fb,
    )
}

/// Bank-0 tile is blank (color 0 → Lightest CRAM); bank-1 tile is color 3 (Darkest).
#[test]
fn cgb_window_uses_bank1_tile() {
    let vram = blank_vram();
    let mut vram1 = blank_vram();
    put_tile_row(&mut vram1, 0x8000, 0, 0xFF, 0xFF);
    vram1[WIN_MAP_ATTR] = 0x08;
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 0, 0x7FFF);
    set_bg_rgb555(&mut cram, 0, 3, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    assert!(render_cgb_window(
        &vram,
        &vram1,
        &cram,
        0,
        0,
        &mut fb,
        &mut colors
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(colors[0], 3);
}

#[test]
fn cgb_window_x_flip_reverses_row() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0x80, 0x80);
    let mut vram1 = blank_vram();
    vram1[WIN_MAP_ATTR] = 0x20;
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 0, 0x7FFF);
    set_bg_rgb555(&mut cram, 0, 3, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    assert!(render_cgb_window(
        &vram,
        &vram1,
        &cram,
        0,
        0,
        &mut fb,
        &mut colors
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
    assert_eq!(fb.pixel(7, 0), Shade::Darkest);
    assert_eq!(colors[0], 0);
    assert_eq!(colors[7], 3);
}

#[test]
fn cgb_window_y_flip_selects_opposite_row() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0x00);
    put_tile_row(&mut vram, 0x8000, 7, 0x00, 0xFF);
    let mut vram1 = blank_vram();
    vram1[WIN_MAP_ATTR] = 0x40;
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 1, 0x7FFF);
    set_bg_rgb555(&mut cram, 0, 2, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    assert!(render_cgb_window(
        &vram,
        &vram1,
        &cram,
        0,
        0,
        &mut fb,
        &mut colors
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(colors[0], 2);
}

#[test]
fn cgb_window_palette1_differs_from_palette0() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let mut vram1 = blank_vram();
    vram1[WIN_MAP_ATTR] = 0x01;
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 3, 0x7FFF);
    set_bg_rgb555(&mut cram, 1, 3, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    assert!(render_cgb_window(
        &vram,
        &vram1,
        &cram,
        0,
        0,
        &mut fb,
        &mut colors
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(colors[0], 3);
}

#[test]
fn cgb_window_priority_does_not_hide_pixels() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let mut vram1 = blank_vram();
    vram1[WIN_MAP_ATTR] = 0x80;
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 3, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];

    assert!(render_cgb_window(
        &vram,
        &vram1,
        &cram,
        0,
        0,
        &mut fb,
        &mut colors
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(colors[0], 3);
}

const LCDC_WIN_NO_BG_ENABLE: u8 = 0b1111_0000; // LCDC.5 set, LCDC.0 clear

#[test]
fn native_cgb_lcdc0_clear_still_draws_window() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let vram1 = blank_vram();
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 3, 0);
    let mut fb = Framebuffer::new();
    let mut colors = [0u8; SCREEN_WIDTH];
    let mut bg_priority = [false; SCREEN_WIDTH];

    assert!(render_scanline(
        &mut WindowLine {
            ly: 0,
            lcdc: LCDC_WIN_NO_BG_ENABLE,
            wx: 7,
            y_active: true,
            win_line: 0,
            bgp: BGP_ID,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: Some(&mut bg_priority),
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Darkest);
    assert_eq!(colors[0], 3);
}

#[test]
fn native_cgb_still_requires_lcdc5_for_window() {
    let mut vram = blank_vram();
    put_tile_row(&mut vram, 0x8000, 0, 0xFF, 0xFF);
    let vram1 = blank_vram();
    let mut cram = Cram::new();
    set_bg_rgb555(&mut cram, 0, 3, 0);
    let mut fb = Framebuffer::new();
    fb.set_pixel(0, 0, Shade::Light);
    let mut colors = [0u8; SCREEN_WIDTH];
    let lcdc = 0b1101_0000; // LCD + tile data; window off, LCDC.0 clear

    assert!(!render_scanline(
        &mut WindowLine {
            ly: 0,
            lcdc,
            wx: 7,
            y_active: true,
            win_line: 0,
            bgp: BGP_ID,
            vram: &vram,
            vram1: Some(&vram1),
            cram: Some(&cram),
            bg_priority: None,
            cgb: None,
            colors: &mut colors,
        },
        &mut fb,
    ));
    assert_eq!(fb.pixel(0, 0), Shade::Light);
}

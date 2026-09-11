//! Background scanline renderer (Phase 5C).
//!
//! Pure: takes LCDC/scroll/BGP + VRAM and writes one visible line into a framebuffer.
//! Timing stays elsewhere.

use super::attr::{BgAttr, shade_from_rgb555};
use super::cram::Cram;
use super::framebuffer::{Framebuffer, SCREEN_WIDTH, Shade, shade_from_bgp};
use super::pixel::CgbPixel;
use super::registers::{LCDC_BG_ENABLE, LCDC_BG_TILE_MAP, LCDC_TILE_DATA};
use super::tile::{decode_tile_row, tile_addr_8000, tile_addr_8800, tile_row_offset, vram_byte};

/// Inputs for one background scanline pass.
pub struct BgLine<'a> {
    pub ly: u8,
    pub lcdc: u8,
    pub scx: u8,
    pub scy: u8,
    pub bgp: u8,
    pub vram: &'a [u8],
    /// VRAM bank 1; `None` keeps the DMG BGP path (no attributes).
    pub vram1: Option<&'a [u8]>,
    pub cram: Option<&'a Cram>,
    pub colors: &'a mut [u8; SCREEN_WIDTH],
    /// Optional CGB attr.7 per pixel (NativeCgb compose).
    pub bg_priority: Option<&'a mut [bool; SCREEN_WIDTH]>,
    /// Optional CGB pixel metadata for Wave 2 mix.
    pub cgb: Option<&'a mut [CgbPixel; SCREEN_WIDTH]>,
}

/// Render background for visible scanline `ly` (0–143) into `fb`.
///
/// Writes color IDs (0–3) into `colors` for OBJ priority. `vram` is `$8000`–`$9FFF`.
pub fn render_scanline(line: &mut BgLine<'_>, fb: &mut Framebuffer) {
    if line.ly >= 144 {
        return;
    }
    // DMG: LCDC.0 clear blanks BG to white. CGB Native (`vram1` Some): still draw;
    // LCDC.0 is master priority at mix time ([Pan Docs LCDC](https://gbdev.io/pandocs/LCDC.html)).
    if line.lcdc & LCDC_BG_ENABLE == 0 && line.vram1.is_none() {
        for (x, color) in line.colors.iter_mut().enumerate() {
            fb.set_pixel(x, line.ly as usize, Shade::Lightest);
            *color = 0;
            if let Some(pri) = line.bg_priority.as_mut() {
                pri[x] = false;
            }
            if let Some(cgb) = line.cgb.as_mut() {
                cgb[x] = CgbPixel::default();
            }
        }
        return;
    }

    let map_base: u16 = if line.lcdc & LCDC_BG_TILE_MAP != 0 {
        0x9C00
    } else {
        0x9800
    };
    let use_8000 = line.lcdc & LCDC_TILE_DATA != 0;

    let bg_y = line.ly.wrapping_add(line.scy);
    let tile_row = u16::from(bg_y / 8);
    let fine_y = bg_y % 8;

    for (screen_x, out_color) in line.colors.iter_mut().enumerate() {
        let bg_x = (line.scx as u16).wrapping_add(screen_x as u16) as u8;
        let tile_col = u16::from(bg_x / 8);
        let fine_x = (bg_x % 8) as usize;

        let map_addr = map_base + tile_row * 32 + tile_col;
        let attr = match line.vram1 {
            Some(bank1) => BgAttr::from_byte(vram_byte(bank1, map_addr)),
            None => BgAttr::default(),
        };
        let fine_x = attr.fine_x(fine_x);
        let fine_y = attr.fine_y(fine_y);

        let tile_id = vram_byte(line.vram, map_addr);
        let tile_base = if use_8000 {
            tile_addr_8000(tile_id)
        } else {
            tile_addr_8800(tile_id)
        };
        let tile_vram = match (attr.tile_bank, line.vram1) {
            (1, Some(bank1)) => bank1,
            _ => line.vram,
        };
        let row_addr = tile_base + tile_row_offset(fine_y) as u16;
        let lo = vram_byte(tile_vram, row_addr);
        let hi = vram_byte(tile_vram, row_addr.wrapping_add(1));
        let row_colors = decode_tile_row(lo, hi);
        let color = row_colors[fine_x];
        *out_color = color;
        let rgb555 = line.cram.map(|c| c.rgb555_bg(attr.palette, color));
        if let Some(pri) = line.bg_priority.as_mut() {
            pri[screen_x] = attr.priority;
        }
        if let Some(cgb) = line.cgb.as_mut() {
            cgb[screen_x] = CgbPixel::from_bg(attr, color, rgb555.unwrap_or(0));
        }
        let shade = if let Some(rgb) = rgb555 {
            shade_from_rgb555(rgb)
        } else {
            shade_from_bgp(color, line.bgp)
        };
        fb.set_pixel(screen_x, line.ly as usize, shade);
    }
}

#[cfg(test)]
mod tests;

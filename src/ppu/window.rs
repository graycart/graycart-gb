//! Window overlay scanline renderer (Phase 5D).
//!
//! Staged: uses the common WX−7 / WY visibility model and an internal line
//! counter that advances only when the window is drawn. Pixel-fetcher / WX=0
//! and WX=166 DMG quirks are deferred (Phase 7).

use super::attr::{BgAttr, shade_from_rgb555};
use super::cram::Cram;
use super::framebuffer::{Framebuffer, SCREEN_WIDTH, shade_from_bgp};
use super::pixel::CgbPixel;
use super::registers::{LCDC_BG_ENABLE, LCDC_TILE_DATA, LCDC_WINDOW_ENABLE, LCDC_WINDOW_TILE_MAP};
use super::tile::{decode_tile_row, tile_addr_8000, tile_addr_8800, tile_row_offset, vram_byte};

/// Inputs for one window overlay pass.
pub struct WindowLine<'a> {
    pub ly: u8,
    pub lcdc: u8,
    pub wx: u8,
    pub y_active: bool,
    pub win_line: u8,
    pub bgp: u8,
    pub vram: &'a [u8],
    /// Bank 1; `None` = DMG path (no attributes).
    pub vram1: Option<&'a [u8]>,
    pub cram: Option<&'a Cram>,
    pub colors: &'a mut [u8; SCREEN_WIDTH],
    pub bg_priority: Option<&'a mut [bool; SCREEN_WIDTH]>,
    pub cgb: Option<&'a mut [CgbPixel; SCREEN_WIDTH]>,
}

/// Whether the window should draw on this scanline (DMG).
///
/// Requires BG/window master enable (LCDC.0), window enable (LCDC.5), the
/// frame’s WY latch (`y_active`), and a sane WX.
pub fn window_visible_on_line(lcdc: u8, y_active: bool, wx: u8) -> bool {
    window_visible_on_line_ex(lcdc, y_active, wx, false)
}

/// CGB Native: LCDC.0 is not a window off switch; LCDC.5 still gates the window.
fn window_visible_on_line_ex(lcdc: u8, y_active: bool, wx: u8, native_cgb: bool) -> bool {
    let bg_enable_ok = native_cgb || lcdc & LCDC_BG_ENABLE != 0;
    bg_enable_ok && lcdc & LCDC_WINDOW_ENABLE != 0 && y_active && wx <= 166
}

/// Overlay the window for one visible scanline.
///
/// Returns `true` if any window pixels were drawn (caller should advance the
/// internal line counter).
pub fn render_scanline(line: &mut WindowLine<'_>, fb: &mut Framebuffer) -> bool {
    let native_cgb = line.vram1.is_some();
    let visible = if native_cgb {
        window_visible_on_line_ex(line.lcdc, line.y_active, line.wx, true)
    } else {
        window_visible_on_line(line.lcdc, line.y_active, line.wx)
    };
    if line.ly >= 144 || !visible {
        return false;
    }

    let win_x0 = i16::from(line.wx) - 7;
    if win_x0 >= SCREEN_WIDTH as i16 {
        return false;
    }

    let map_base: u16 = if line.lcdc & LCDC_WINDOW_TILE_MAP != 0 {
        0x9C00
    } else {
        0x9800
    };
    let use_8000 = line.lcdc & LCDC_TILE_DATA != 0;

    let tile_row = u16::from(line.win_line / 8);
    let fine_y = line.win_line % 8;
    let start_x = win_x0.max(0) as usize;

    for screen_x in start_x..SCREEN_WIDTH {
        let win_x = (screen_x as i16 - win_x0) as u8;
        let tile_col = u16::from(win_x / 8);
        let fine_x = (win_x % 8) as usize;

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
        let tile_vram = if attr.tile_bank == 1 {
            line.vram1.unwrap_or(line.vram)
        } else {
            line.vram
        };
        let row_addr = tile_base + tile_row_offset(fine_y) as u16;
        let lo = vram_byte(tile_vram, row_addr);
        let hi = vram_byte(tile_vram, row_addr.wrapping_add(1));
        let row_colors = decode_tile_row(lo, hi);
        let color = row_colors[fine_x];
        line.colors[screen_x] = color;
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
    true
}

#[cfg(test)]
mod tests;

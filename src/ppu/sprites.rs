//! OBJ (sprite) scanline composition (Phase 5E / 10E).

use super::cram::Cram;
use super::framebuffer::{Framebuffer, SCREEN_WIDTH, shade_from_bgp};
use super::oam::{Oam, SPRITE_COUNT, SPRITES_PER_LINE, Sprite};
use super::obj_attr::ObjAttr;
use super::registers::{LCDC_OBJ_ENABLE, LCDC_OBJ_SIZE};
use super::tile::{decode_tile_row, tile_addr_8000, tile_row_offset, vram_byte};

pub use super::compose::CgbObjSample;

/// Inputs for one sprite overlay pass.
///
/// DMG: [`render_scanline`]. CGB: [`collect_cgb_obj_line`].
pub struct SpriteLine<'a> {
    pub ly: u8,
    pub lcdc: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub oam: &'a Oam,
    pub vram: &'a [u8],
    pub bg_colors: &'a [u8; SCREEN_WIDTH],
    pub vram1: Option<&'a [u8]>,
    pub cram: Option<&'a Cram>,
}

/// Collect up to 10 sprites intersecting `ly` (OAM order), then sort by X (DMG).
pub fn sprites_for_line(oam: &Oam, ly: u8, height: u8) -> Vec<(usize, Sprite)> {
    sprites_for_line_mode(oam, ly, height, false)
}

/// Same 10-per-line Y scan as DMG. `cgb_obj` skips the X sort (OAM discovery order).
pub fn sprites_for_line_mode(oam: &Oam, ly: u8, height: u8, cgb_obj: bool) -> Vec<(usize, Sprite)> {
    let mut hit = Vec::with_capacity(SPRITES_PER_LINE);
    for i in 0..SPRITE_COUNT {
        let Some(sp) = oam.sprite(i) else {
            break;
        };
        if sp.intersects_line(ly, height) {
            hit.push((i, sp));
            if hit.len() == SPRITES_PER_LINE {
                break;
            }
        }
    }
    if !cgb_obj {
        // Non-CGB / OPRI=1: lower X wins; equal X → earlier OAM (stable sort by X).
        hit.sort_by(|a, b| a.1.x.cmp(&b.1.x).then_with(|| a.0.cmp(&b.0)));
    }
    hit
}

/// Draw sprites onto the scanline. `bg_colors` holds BG/window color IDs (0–3)
/// for OBJ-to-BG priority.
pub fn render_scanline(line: &SpriteLine<'_>, fb: &mut Framebuffer) {
    if line.lcdc & LCDC_OBJ_ENABLE == 0 || line.ly >= 144 {
        return;
    }
    let height: u8 = if line.lcdc & LCDC_OBJ_SIZE != 0 {
        16
    } else {
        8
    };
    // DMG only: CGB Native uses [`collect_cgb_obj_line`]. Never OAM-order+OBP here.
    let sprites = sprites_for_line(line.oam, line.ly, height);

    // Draw back-to-front so earlier (higher priority) sprites overwrite.
    for &(_, sp) in sprites.iter().rev() {
        let row = sp.row_in_sprite(line.ly, height);
        let tile_id = if height == 16 {
            if row < 8 {
                sp.tile & 0xFE
            } else {
                sp.tile | 0x01
            }
        } else {
            sp.tile
        };
        let row_in_tile = row & 7;
        let tile_base = tile_addr_8000(tile_id);
        let row_addr = tile_base + tile_row_offset(row_in_tile) as u16;
        let lo = vram_byte(line.vram, row_addr);
        let hi = vram_byte(line.vram, row_addr.wrapping_add(1));
        let mut colors = decode_tile_row(lo, hi);
        if sp.flip_x() {
            colors.reverse();
        }
        let pal = if sp.obp1() { line.obp1 } else { line.obp0 };
        let screen_x0 = i16::from(sp.x) - 8;

        for (fine_x, &color) in colors.iter().enumerate() {
            if color == 0 {
                continue; // transparent
            }
            let sx = screen_x0 + fine_x as i16;
            if !(0..SCREEN_WIDTH as i16).contains(&sx) {
                continue;
            }
            let x = sx as usize;
            if sp.priority_behind_bg() && line.bg_colors[x] != 0 {
                continue;
            }
            fb.set_pixel(x, line.ly as usize, shade_from_bgp(color, pal));
        }
    }
}

/// OBJ-vs-OBJ only: earlier OAM opaque pixel wins. Color 0 is transparent.
/// Does not consult `bg_colors` or apply BG-over-OBJ.
pub fn collect_cgb_obj_line(line: &SpriteLine<'_>) -> [Option<CgbObjSample>; SCREEN_WIDTH] {
    let mut out = [None; SCREEN_WIDTH];
    if line.lcdc & LCDC_OBJ_ENABLE == 0 || line.ly >= 144 {
        return out;
    }
    let Some(cram) = line.cram else {
        return out;
    };
    let height: u8 = if line.lcdc & LCDC_OBJ_SIZE != 0 {
        16
    } else {
        8
    };
    let sprites = sprites_for_line_mode(line.oam, line.ly, height, true);

    for &(_, sp) in &sprites {
        let attr = ObjAttr::from_byte(sp.attrs);
        let row = sp.row_in_sprite(line.ly, height);
        let tile_id = if height == 16 {
            if row < 8 {
                sp.tile & 0xFE
            } else {
                sp.tile | 0x01
            }
        } else {
            sp.tile
        };
        let row_in_tile = row & 7;
        let tile_base = tile_addr_8000(tile_id);
        let row_addr = tile_base + tile_row_offset(row_in_tile) as u16;
        let bank: &[u8] = if attr.tile_bank == 1 {
            match line.vram1 {
                Some(bank1) => bank1,
                None => continue,
            }
        } else {
            line.vram
        };
        let lo = vram_byte(bank, row_addr);
        let hi = vram_byte(bank, row_addr.wrapping_add(1));
        let mut colors = decode_tile_row(lo, hi);
        if attr.x_flip {
            colors.reverse();
        }
        let screen_x0 = i16::from(sp.x) - 8;

        for (fine_x, &color) in colors.iter().enumerate() {
            if color == 0 {
                continue;
            }
            let sx = screen_x0 + fine_x as i16;
            if !(0..SCREEN_WIDTH as i16).contains(&sx) {
                continue;
            }
            let x = sx as usize;
            if out[x].is_some() {
                continue;
            }
            out[x] = Some(CgbObjSample {
                color_id: color,
                rgb555: cram.rgb555_obj(attr.palette, color),
                bg_over: attr.priority_bg_over_obj,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests;

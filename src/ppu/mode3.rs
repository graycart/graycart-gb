//! Mode 3 (pixel transfer) duration — base 172 dots plus Pan Docs penalties.
//!
//! Penalties shorten Mode 0 on the same scanline (line length stays 456).

use super::oam::{Oam, SPRITE_COUNT, SPRITES_PER_LINE, Sprite};
use super::registers::{LCDC_OBJ_ENABLE, LCDC_OBJ_SIZE, LCDC_WINDOW_ENABLE, Registers};
use super::timing::MODE3_DOTS;

/// Minimum Mode 3 length (160 pixels + 12 fetcher dots).
pub const MODE3_BASE: u32 = MODE3_DOTS;

/// Mode 3 length for `ly` given current LCD regs / OAM.
///
/// Samples SCX, window enable, and sprites at Mode 3 entry (end of Mode 2).
pub fn mode3_length(ly: u8, regs: &Registers, oam: &Oam, window_y_active: bool) -> u32 {
    let mut dots = MODE3_BASE;
    dots += u32::from(regs.scx % 8);

    if window_penalty(ly, regs, window_y_active) {
        dots += 6;
    }

    if regs.lcdc & LCDC_OBJ_ENABLE != 0 {
        let height: u8 = if regs.lcdc & LCDC_OBJ_SIZE != 0 {
            16
        } else {
            8
        };
        dots += obj_penalties(ly, regs.scx, oam, height);
    }

    dots.min(289) // Pan Docs upper bound
}

fn window_penalty(ly: u8, regs: &Registers, window_y_active: bool) -> bool {
    if regs.lcdc & LCDC_WINDOW_ENABLE == 0 {
        return false;
    }
    let y_ok = window_y_active || regs.wy == ly;
    if !y_ok {
        return false;
    }
    // WX=0..166 can trigger on hardware; out-of-range WX skips the window.
    regs.wx <= 166
}

/// OBJ Mode 3 penalties (Pan Docs “OBJ penalty algorithm”), OAM-order then X sort.
fn obj_penalties(ly: u8, scx: u8, oam: &Oam, height: u8) -> u32 {
    let mut hit: Vec<(usize, Sprite)> = Vec::with_capacity(SPRITES_PER_LINE);
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
    // Same selection order as rendering: OAM discovery order, then X (then index).
    hit.sort_by(|a, b| a.1.x.cmp(&b.1.x).then_with(|| a.0.cmp(&b.0)));

    let mut penalty = 0u32;
    let mut considered_tiles = [false; 32]; // coarse: screen may span ~21 tiles + scroll

    for &(_, sp) in &hit {
        if sp.x == 0 {
            // Completely off the left edge: fixed 11-dot penalty.
            penalty += 11;
            continue;
        }

        // “The Pixel” = OBJ’s leftmost pixel screen X = OAM X − 8.
        let pixel_x = i16::from(sp.x) - 8;
        // Map into BG space (ignore window shift for tile-id; approx with SCX).
        let bg_x = pixel_x.wrapping_add(i16::from(scx));
        let tile_idx = ((bg_x.rem_euclid(256)) as u16 / 8) as usize % considered_tiles.len();
        let fine = (bg_x.rem_euclid(8)) as u32;

        if !considered_tiles[tile_idx] {
            considered_tiles[tile_idx] = true;
            // Pixels of the tile strictly to the right of The Pixel, minus 2.
            let to_right = 7u32.saturating_sub(fine);
            penalty += to_right.saturating_sub(2);
        }
        penalty += 6; // OBJ tile fetch
    }

    penalty
}

#[cfg(test)]
mod tests;

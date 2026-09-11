//! CGB scanline compose: BG/window vs one resolved OBJ pixel.
//!
//! [Pan Docs Tile Maps](https://gbdev.io/pandocs/Tile_Maps.html) BG-to-OBJ
//! priority; LCDC.0 is master priority in CGB mode
//! ([LCDC](https://gbdev.io/pandocs/LCDC.html)).

use super::framebuffer::SCREEN_WIDTH;
use super::pixel::CgbPixel;
use super::priority::cgb_obj_wins_over_bg;

/// One OBJ sample after OBJ-vs-OBJ (Task F). `bg_over` is OAM attr.7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CgbObjSample {
    pub color_id: u8,
    pub rgb555: u16,
    pub bg_over: bool,
}

/// Mix one Native CGB scanline. Transparent OBJ (`None` or color 0) keeps BG.
pub fn mix_cgb_line(
    bg: &[CgbPixel; SCREEN_WIDTH],
    obj: &[Option<CgbObjSample>; SCREEN_WIDTH],
    lcdc0: bool,
) -> [CgbPixel; SCREEN_WIDTH] {
    let mut out = *bg;
    for (x, sample) in obj.iter().enumerate() {
        out[x] = mix_pixel(bg[x], *sample, lcdc0);
    }
    out
}

fn mix_pixel(bg: CgbPixel, obj: Option<CgbObjSample>, lcdc0: bool) -> CgbPixel {
    let Some(obj) = obj else {
        return bg;
    };
    let obj_px = CgbPixel::from_obj(obj.color_id, obj.rgb555);
    if obj_px.is_transparent() {
        return bg;
    }
    if cgb_obj_wins_over_bg(bg.color_id, bg.bg_priority, obj.bg_over, lcdc0) {
        obj_px
    } else {
        bg
    }
}

#[cfg(test)]
mod tests;

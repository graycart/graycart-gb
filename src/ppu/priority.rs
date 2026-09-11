//! CGB BG-vs-OBJ pixel priority ([Pan Docs Tile Maps](https://gbdev.io/pandocs/Tile_Maps.html)).
//!
//! OBJ-vs-OBJ (OAM order / OPRI) is resolved first to one object pixel; this
//! module only decides BG vs that object. Wave 2 wires the table into compose.

/// Returns `true` when the object pixel is drawn instead of the BG/window pixel.
///
/// `obj_bg_over` is OAM attribute bit 7: set means “BG over this OBJ”.
pub fn cgb_obj_wins_over_bg(
    bg_color_id: u8,
    bg_attr_priority: bool,
    obj_bg_over: bool,
    lcdc0: bool,
) -> bool {
    // Pan Docs Tile Maps — BG-to-OBJ Priority in CGB Mode:
    // 1. BG color index 0 ⇒ OBJ
    // 2. else LCDC.0 clear ⇒ OBJ (master priority off)
    // 3. else both BG attr.7 and OAM attr.7 clear ⇒ OBJ
    // 4. else BG (indices 1–3)
    bg_color_id == 0 || !lcdc0 || (!bg_attr_priority && !obj_bg_over)
}

#[cfg(test)]
mod tests;

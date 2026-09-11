use super::ObjAttr;

#[test]
fn default_is_bank0_palette0_no_flips_no_priority() {
    assert_eq!(
        ObjAttr::default(),
        ObjAttr {
            priority_bg_over_obj: false,
            y_flip: false,
            x_flip: false,
            tile_bank: 0,
            palette: 0,
        }
    );
}

#[test]
fn from_byte_zero_matches_default() {
    assert_eq!(ObjAttr::from_byte(0x00), ObjAttr::default());
}

#[test]
fn from_byte_decodes_priority_flips_bank_and_palette() {
    // bits 7–5, 3, 2–0 set; bit 4 (Non-CGB OBP) left clear
    let a = ObjAttr::from_byte(0b1110_1111);
    assert!(a.priority_bg_over_obj);
    assert!(a.y_flip);
    assert!(a.x_flip);
    assert_eq!(a.tile_bank, 1);
    assert_eq!(a.palette, 7);
}

#[test]
fn from_byte_ignores_bit4_dmg_obp() {
    assert_eq!(ObjAttr::from_byte(0x10), ObjAttr::from_byte(0x00));
    let with = ObjAttr::from_byte(0x18);
    let without = ObjAttr::from_byte(0x08);
    assert_eq!(with, without);
    assert_eq!(with.tile_bank, 1);
    assert_eq!(with.palette, 0);

    let pal7_obp = ObjAttr::from_byte(0x17);
    assert_eq!(pal7_obp.palette, 7);
    assert_eq!(pal7_obp.tile_bank, 0);
    assert!(!pal7_obp.priority_bg_over_obj);
}

#[test]
fn from_byte_single_flag_bits() {
    assert!(ObjAttr::from_byte(0x80).priority_bg_over_obj);
    assert!(!ObjAttr::from_byte(0x00).priority_bg_over_obj);
    assert!(ObjAttr::from_byte(0x40).y_flip);
    assert!(ObjAttr::from_byte(0x20).x_flip);
    assert_eq!(ObjAttr::from_byte(0x08).tile_bank, 1);
    assert_eq!(ObjAttr::from_byte(0x00).tile_bank, 0);
    assert_eq!(ObjAttr::from_byte(0x05).palette, 5);
    assert_eq!(ObjAttr::from_byte(0x07).palette, 7);
}

#[test]
fn from_byte_priority_set_and_clear() {
    assert!(ObjAttr::from_byte(0x80).priority_bg_over_obj);
    assert!(!ObjAttr::from_byte(0x7F).priority_bg_over_obj);
}

#[test]
fn fine_x_masks_and_flips() {
    let plain = ObjAttr::from_byte(0x00);
    assert_eq!(plain.fine_x(0), 0);
    assert_eq!(plain.fine_x(7), 7);
    assert_eq!(plain.fine_x(8), 0);

    let flipped = ObjAttr::from_byte(0x20);
    assert_eq!(flipped.fine_x(0), 7);
    assert_eq!(flipped.fine_x(7), 0);
    assert_eq!(flipped.fine_x(8), 7);
}

#[test]
fn fine_y_masks_and_flips() {
    let plain = ObjAttr::from_byte(0x00);
    assert_eq!(plain.fine_y(0), 0);
    assert_eq!(plain.fine_y(7), 7);
    assert_eq!(plain.fine_y(8), 0);

    let flipped = ObjAttr::from_byte(0x40);
    assert_eq!(flipped.fine_y(0), 7);
    assert_eq!(flipped.fine_y(7), 0);
    assert_eq!(flipped.fine_y(8), 7);
}

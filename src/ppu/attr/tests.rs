use super::{BgAttr, shade_from_rgb555};
use crate::ppu::framebuffer::Shade;

#[test]
fn default_is_bank0_palette0_no_flips() {
    assert_eq!(
        BgAttr::default(),
        BgAttr {
            priority: false,
            y_flip: false,
            x_flip: false,
            tile_bank: 0,
            palette: 0,
        }
    );
}

#[test]
fn from_byte_zero_matches_default() {
    assert_eq!(BgAttr::from_byte(0x00), BgAttr::default());
}

#[test]
fn from_byte_decodes_priority_flips_bank_and_palette() {
    let a = BgAttr::from_byte(0b1110_1111);
    assert!(a.priority);
    assert!(a.y_flip);
    assert!(a.x_flip);
    assert_eq!(a.tile_bank, 1);
    assert_eq!(a.palette, 7);
}

#[test]
fn from_byte_ignores_unused_bit4() {
    assert_eq!(BgAttr::from_byte(0x10), BgAttr::from_byte(0x00));
    let with = BgAttr::from_byte(0x18);
    let without = BgAttr::from_byte(0x08);
    assert_eq!(with, without);
    assert_eq!(with.tile_bank, 1);
}

#[test]
fn from_byte_single_flag_bits() {
    assert!(BgAttr::from_byte(0x80).priority);
    assert!(BgAttr::from_byte(0x40).y_flip);
    assert!(BgAttr::from_byte(0x20).x_flip);
    assert_eq!(BgAttr::from_byte(0x08).tile_bank, 1);
    assert_eq!(BgAttr::from_byte(0x05).palette, 5);
}

#[test]
fn fine_x_masks_and_flips() {
    let plain = BgAttr::from_byte(0x00);
    assert_eq!(plain.fine_x(0), 0);
    assert_eq!(plain.fine_x(7), 7);
    assert_eq!(plain.fine_x(8), 0);

    let flipped = BgAttr::from_byte(0x20);
    assert_eq!(flipped.fine_x(0), 7);
    assert_eq!(flipped.fine_x(7), 0);
    assert_eq!(flipped.fine_x(8), 7);
}

#[test]
fn fine_y_masks_and_flips() {
    let plain = BgAttr::from_byte(0x00);
    assert_eq!(plain.fine_y(0), 0);
    assert_eq!(plain.fine_y(7), 7);
    assert_eq!(plain.fine_y(8), 0);

    let flipped = BgAttr::from_byte(0x40);
    assert_eq!(flipped.fine_y(0), 7);
    assert_eq!(flipped.fine_y(7), 0);
    assert_eq!(flipped.fine_y(8), 7);
}

#[test]
fn shade_from_rgb555_black_is_darkest_white_is_lightest() {
    assert_eq!(shade_from_rgb555(0), Shade::Darkest);
    assert_eq!(shade_from_rgb555(0x7FFF), Shade::Lightest);
    assert_eq!(shade_from_rgb555(0xFFFF), Shade::Lightest);
}

#[test]
fn shade_from_rgb555_mid_green_is_distinguishable() {
    let mid_green = 0x03E0; // R=0, G=31, B=0
    let shade = shade_from_rgb555(mid_green);
    assert_ne!(shade, Shade::Darkest);
    assert_ne!(shade, Shade::Lightest);
}

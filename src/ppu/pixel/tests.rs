use super::{CgbPixel, PixelSource};
use crate::ppu::attr::BgAttr;

#[test]
fn stores_color_id_zero_and_nonzero() {
    let zero = CgbPixel::new(0, false, 0x1234, PixelSource::Bg);
    assert_eq!(zero.color_id, 0);

    let one = CgbPixel::new(1, false, 0x1234, PixelSource::Bg);
    let two = CgbPixel::new(2, false, 0x1234, PixelSource::Bg);
    let three = CgbPixel::new(3, false, 0x1234, PixelSource::Bg);
    assert_eq!(one.color_id, 1);
    assert_eq!(two.color_id, 2);
    assert_eq!(three.color_id, 3);
}

#[test]
fn stores_bg_priority_flag() {
    let off = CgbPixel::new(1, false, 0, PixelSource::Bg);
    let on = CgbPixel::new(1, true, 0, PixelSource::Bg);
    assert!(!off.bg_priority);
    assert!(on.bg_priority);
}

#[test]
fn stores_rgb555() {
    let p = CgbPixel::new(2, false, 0x7ABC, PixelSource::Obj);
    assert_eq!(p.rgb555, 0x7ABC);
}

#[test]
fn default_is_transparent_bg_with_no_priority() {
    let p = CgbPixel::default();
    assert_eq!(p.color_id, 0);
    assert!(!p.bg_priority);
    assert_eq!(p.rgb555, 0);
    assert_eq!(p.source, PixelSource::Bg);
    assert!(p.is_transparent());
}

#[test]
fn color_id_zero_is_transparent_nonzero_is_not() {
    assert!(CgbPixel::new(0, true, 0x7FFF, PixelSource::Obj).is_transparent());
    assert!(!CgbPixel::new(1, false, 0, PixelSource::Bg).is_transparent());
    assert!(!CgbPixel::new(3, true, 0, PixelSource::Obj).is_transparent());
}

#[test]
fn from_bg_copies_attr_priority_and_marks_bg_source() {
    let attr = BgAttr::from_byte(0x80);
    let p = CgbPixel::from_bg(attr, 2, 0x001F);
    assert_eq!(p.color_id, 2);
    assert!(p.bg_priority);
    assert_eq!(p.rgb555, 0x001F);
    assert_eq!(p.source, PixelSource::Bg);
}

#[test]
fn from_bg_clears_priority_when_attr7_clear() {
    let attr = BgAttr::from_byte(0x07);
    let p = CgbPixel::from_bg(attr, 1, 0x03E0);
    assert!(!p.bg_priority);
    assert_eq!(p.source, PixelSource::Bg);
}

#[test]
fn obj_pixel_has_no_bg_priority() {
    let p = CgbPixel::from_obj(3, 0x7C00);
    assert_eq!(p.color_id, 3);
    assert!(!p.bg_priority);
    assert_eq!(p.rgb555, 0x7C00);
    assert_eq!(p.source, PixelSource::Obj);
}

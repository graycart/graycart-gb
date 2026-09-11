//! CGB compose: Pan Docs four BG-vs-OBJ rules on `CgbPixel` lines.

use super::{CgbObjSample, mix_cgb_line};
use crate::ppu::framebuffer::SCREEN_WIDTH;
use crate::ppu::pixel::{CgbPixel, PixelSource};

const BG_RGB: u16 = 0x001F;
const OBJ_RGB: u16 = 0x7C00;

fn bg_line(color_id: u8, bg_priority: bool) -> [CgbPixel; SCREEN_WIDTH] {
    [CgbPixel::new(color_id, bg_priority, BG_RGB, PixelSource::Bg); SCREEN_WIDTH]
}

fn obj_line(color_id: u8, bg_over: bool) -> [Option<CgbObjSample>; SCREEN_WIDTH] {
    [Some(CgbObjSample {
        color_id,
        rgb555: OBJ_RGB,
        bg_over,
    }); SCREEN_WIDTH]
}

fn none_obj() -> [Option<CgbObjSample>; SCREEN_WIDTH] {
    [None; SCREEN_WIDTH]
}

fn assert_all_obj(out: &[CgbPixel; SCREEN_WIDTH]) {
    for p in out {
        assert_eq!(p.color_id, 3);
        assert_eq!(p.rgb555, OBJ_RGB);
        assert_eq!(p.source, PixelSource::Obj);
        assert!(!p.bg_priority);
    }
}

fn assert_all_bg(out: &[CgbPixel; SCREEN_WIDTH], color_id: u8, bg_priority: bool) {
    for p in out {
        assert_eq!(p.color_id, color_id);
        assert_eq!(p.bg_priority, bg_priority);
        assert_eq!(p.rgb555, BG_RGB);
        assert_eq!(p.source, PixelSource::Bg);
    }
}

#[test]
fn none_obj_keeps_bg() {
    let bg = bg_line(2, true);
    let out = mix_cgb_line(&bg, &none_obj(), true);
    assert_all_bg(&out, 2, true);
}

#[test]
fn obj_color_0_keeps_bg() {
    let bg = bg_line(2, false);
    let out = mix_cgb_line(&bg, &obj_line(0, false), true);
    assert_all_bg(&out, 2, false);
}

/// Rule 1: BG color index 0 ⇒ OBJ wins (any flags).
#[test]
fn rule1_bg_color_0_obj_wins() {
    let bg = bg_line(0, true);
    let out = mix_cgb_line(&bg, &obj_line(3, true), true);
    assert_all_obj(&out);
}

/// Rule 2: LCDC.0 clear ⇒ OBJ wins for BG indices 1–3.
#[test]
fn rule2_lcdc0_clear_obj_wins() {
    let bg = bg_line(2, true);
    let out = mix_cgb_line(&bg, &obj_line(3, true), false);
    assert_all_obj(&out);
}

/// Rule 3: LCDC.0 set and both attr.7 clear ⇒ OBJ wins.
#[test]
fn rule3_both_priority_bits_clear_obj_wins() {
    let bg = bg_line(1, false);
    let out = mix_cgb_line(&bg, &obj_line(3, false), true);
    assert_all_obj(&out);
}

/// Rule 4: LCDC.0 set and either attr.7 set ⇒ BG wins for indices 1–3.
#[test]
fn rule4_either_priority_bit_bg_wins() {
    let bg_pri = bg_line(2, true);
    let out = mix_cgb_line(&bg_pri, &obj_line(3, false), true);
    assert_all_bg(&out, 2, true);

    let oam_pri = bg_line(3, false);
    let out = mix_cgb_line(&oam_pri, &obj_line(3, true), true);
    assert_all_bg(&out, 3, false);
}

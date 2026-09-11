use super::*;

#[test]
fn framebuffer_starts_lightest_and_round_trips() {
    let mut fb = Framebuffer::new();
    assert_eq!(fb.pixels().len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
    assert_eq!(fb.rgb555_pixel(0, 0), 0x7FFF);
    fb.set_pixel(159, 143, Shade::Darkest);
    assert_eq!(fb.pixel(159, 143), Shade::Darkest);
    fb.clear(Shade::Dark);
    assert_eq!(fb.pixel(0, 0), Shade::Dark);
    assert_eq!(fb.pixel(159, 143), Shade::Dark);
}

#[test]
fn bgp_maps_color_ids_to_shades() {
    // Classic DMG: 0→0, 1→1, 2→2, 3→3 encoded as $E4
    let bgp = 0b11_10_01_00;
    assert_eq!(shade_from_bgp(0, bgp), Shade::Lightest);
    assert_eq!(shade_from_bgp(1, bgp), Shade::Light);
    assert_eq!(shade_from_bgp(2, bgp), Shade::Dark);
    assert_eq!(shade_from_bgp(3, bgp), Shade::Darkest);

    // Inverted: color 0 → darkest
    assert_eq!(shade_from_bgp(0, 0b00_00_00_11), Shade::Darkest);
}

#[test]
fn out_of_bounds_set_pixel_is_ignored() {
    let mut fb = Framebuffer::new();
    fb.set_pixel(160, 0, Shade::Darkest);
    fb.set_pixel(0, 144, Shade::Darkest);
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
}

#[test]
fn set_cgb_pixel_round_trips_shade_and_rgb555() {
    let mut fb = Framebuffer::new();
    fb.set_cgb_pixel(3, 4, Shade::Dark, 0x001F);
    assert_eq!(fb.pixel(3, 4), Shade::Dark);
    assert_eq!(fb.rgb555_pixel(3, 4), 0x001F);
    fb.set_cgb_pixel(160, 0, Shade::Darkest, 0x7FFF);
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
    assert_eq!(fb.rgb555_pixel(0, 0), 0x7FFF);
}

#[test]
fn set_pixel_does_not_imply_presents_cgb_color() {
    let mut fb = Framebuffer::new();
    assert!(!fb.presents_cgb_color());
    fb.set_pixel(0, 0, Shade::Darkest);
    assert!(!fb.presents_cgb_color());
    fb.set_presents_cgb_color(true);
    assert!(fb.presents_cgb_color());
}

#[test]
fn clear_fills_rgb555_white_for_lightest() {
    let mut fb = Framebuffer::new();
    fb.set_cgb_pixel(0, 0, Shade::Darkest, 0);
    fb.clear(Shade::Lightest);
    assert_eq!(fb.pixel(0, 0), Shade::Lightest);
    assert_eq!(fb.rgb555_pixel(0, 0), 0x7FFF);
    fb.clear(Shade::Darkest);
    assert_eq!(fb.rgb555_pixel(0, 0), 0);
}

use super::{BGPD, BGPI, Cram, OBPD, OBPI};

#[test]
fn mmio_addresses_match_pan_docs() {
    assert_eq!(BGPI, 0xFF68);
    assert_eq!(BGPD, 0xFF69);
    assert_eq!(OBPI, 0xFF6A);
    assert_eq!(OBPD, 0xFF6B);
}

#[test]
fn power_on_is_zero_ram_address_zero_auto_inc_off() {
    let cram = Cram::new();
    assert_eq!(cram.read_bgpi(), 0x40);
    assert_eq!(cram.read_obpi(), 0x40);
    assert_eq!(cram.read_bgpd(true), 0x00);
    assert_eq!(cram.read_obpd(true), 0x00);
    assert_eq!(cram.rgb555_bg(0, 0), 0);
    assert_eq!(cram.rgb555_obj(0, 0), 0);
}

#[test]
fn index_write_sets_auto_inc_and_address_bit6_reads_as_one() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80 | 0x05);
    assert_eq!(cram.read_bgpi(), 0xC5);

    cram.write_bgpi(0x40 | 0x05);
    assert_eq!(cram.read_bgpi(), 0x45);
    cram.write_bgpd(0xAA, true);
    assert_eq!(cram.read_bgpi(), 0x45);
    assert_eq!(cram.read_bgpd(true), 0xAA);
}

#[test]
fn wrap_from_63_keeps_auto_inc_bit() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80 | 63);
    cram.write_bgpd(0x12, true);
    assert_eq!(cram.read_bgpi(), 0xC0);
    cram.write_bgpi(0x80 | 63);
    assert_eq!(cram.read_bgpd(true), 0x12);
}

#[test]
fn data_write_increments_data_read_does_not() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80);
    cram.write_bgpd(0x11, true);
    assert_eq!(cram.read_bgpi(), 0xC1);
    assert_eq!(cram.read_bgpd(true), 0x00);
    assert_eq!(cram.read_bgpi(), 0xC1);
    cram.write_bgpd(0x22, true);
    assert_eq!(cram.read_bgpi(), 0xC2);

    cram.write_bgpi(0x80);
    assert_eq!(cram.read_bgpd(true), 0x11);
    cram.write_bgpi(0x81);
    assert_eq!(cram.read_bgpd(true), 0x22);
}

#[test]
fn inaccessible_write_does_not_store_but_increments() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80);
    cram.write_bgpd(0x33, false);
    assert_eq!(cram.read_bgpi(), 0xC1);
    cram.write_bgpi(0x80);
    assert_eq!(cram.read_bgpd(true), 0x00);
}

#[test]
fn inaccessible_read_is_ff_without_increment() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80);
    cram.write_bgpd(0x44, true);
    cram.write_bgpi(0x80);
    assert_eq!(cram.read_bgpd(false), 0xFF);
    assert_eq!(cram.read_bgpi(), 0xC0);
    assert_eq!(cram.read_bgpd(true), 0x44);
}

#[test]
fn bg_and_obj_banks_are_independent() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80);
    cram.write_bgpd(0xAB, true);
    cram.write_obpi(0x80);
    cram.write_obpd(0xCD, true);

    cram.write_bgpi(0x00);
    cram.write_obpi(0x00);
    assert_eq!(cram.read_bgpd(true), 0xAB);
    assert_eq!(cram.read_obpd(true), 0xCD);

    cram.write_bgpi(0x00);
    cram.write_bgpd(0x11, true);
    cram.write_obpi(0x00);
    assert_eq!(cram.read_obpd(true), 0xCD);
    cram.write_obpi(0x00);
    cram.write_obpd(0x22, true);
    cram.write_bgpi(0x00);
    assert_eq!(cram.read_bgpd(true), 0x11);
}

#[test]
fn rgb555_little_endian_preserves_bit_15() {
    let mut cram = Cram::new();
    cram.write_bgpi(0x80);
    cram.write_bgpd(0xF0, true);
    cram.write_bgpd(0x7C, true);
    assert_eq!(cram.rgb555_bg(0, 0), 0x7CF0);

    cram.write_bgpi(0x80);
    cram.write_bgpd(0xF0, true);
    cram.write_bgpd(0xFF, true);
    assert_eq!(cram.rgb555_bg(0, 0), 0xFFF0);

    cram.write_obpi(0x80);
    cram.write_obpd(0xF0, true);
    cram.write_obpd(0x7C, true);
    assert_eq!(cram.rgb555_obj(0, 0), 0x7CF0);
}

#[test]
fn new_cram_is_all_zero() {
    let cram = Cram::new();
    for pal in 0..8 {
        for color in 0..4 {
            assert_eq!(cram.rgb555_bg(pal, color), 0);
            assert_eq!(cram.rgb555_obj(pal, color), 0);
        }
    }
}

const WHITE: u16 = 0x7FFF;
const GRAY1: u16 = 0x5294;
const GRAY2: u16 = 0x294A;
const BLACK: u16 = 0x0000;

#[test]
fn fill_bg_white_sets_all_bg_colors_obj_untouched() {
    let mut cram = Cram::new();
    cram.fill_bg_white();
    assert_eq!(cram.rgb555_bg(0, 0), WHITE);
    for pal in 0..8 {
        for color in 0..4 {
            assert_eq!(cram.rgb555_bg(pal, color), WHITE);
            assert_eq!(cram.rgb555_obj(pal, color), 0);
        }
    }
}

#[test]
fn fill_compat_gray_ramp_fills_bg0_and_obj0_1_only() {
    let mut cram = Cram::new();
    cram.fill_compat_gray_ramp();

    let ramp = [WHITE, GRAY1, GRAY2, BLACK];
    for color in 0..4 {
        assert_eq!(cram.rgb555_bg(0, color), ramp[color as usize]);
        assert_eq!(cram.rgb555_obj(0, color), ramp[color as usize]);
        assert_eq!(cram.rgb555_obj(1, color), ramp[color as usize]);
    }
    for pal in 1..8 {
        for color in 0..4 {
            assert_eq!(cram.rgb555_bg(pal, color), 0xFFFF);
        }
    }
    for pal in 2..8 {
        for color in 0..4 {
            assert_eq!(cram.rgb555_obj(pal, color), 0xFFFF);
        }
    }
}

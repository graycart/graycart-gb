use super::*;

#[test]
fn daa_after_bcd_add_adjusts_low_nibble() {
    // 0x09 + 0x01 = 0x0A with H; DAA → 0x10
    let r = daa(0x0A, false, true, false);
    assert_eq!(r.value, 0x10);
    assert!(!r.z);
    assert!(!r.n);
    assert!(!r.h);
    assert!(!r.c);
}

#[test]
fn daa_after_bcd_add_sets_carry_past_99() {
    let r = daa(0x9A, false, false, false);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);
}

#[test]
fn daa_after_bcd_sub_uses_n_flag() {
    let r = daa(0x00, true, true, true);
    assert_eq!(r.value, 0x9A);
    assert!(r.n);
    assert!(!r.h);
    assert!(r.c);
}

#[test]
fn add_sets_half_and_full_carry() {
    let r = add8(0x0F, 0x01);
    assert_eq!(r.value, 0x10);
    assert!(r.h);
    assert!(!r.c);
    assert!(!r.z);
    assert!(!r.n);

    let r = add8(0xFF, 0x01);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.h);
    assert!(r.c);
}

#[test]
fn adc_includes_carry() {
    let r = adc8(0x0F, 0x00, true);
    assert_eq!(r.value, 0x10);
    assert!(r.h);
}

#[test]
fn sub_and_sbc_borrow() {
    let r = sub8(0x10, 0x01);
    assert_eq!(r.value, 0x0F);
    assert!(r.n);
    assert!(r.h);
    assert!(!r.c);

    let r = sbc8(0x00, 0x00, true);
    assert_eq!(r.value, 0xFF);
    assert!(!r.z);
    assert!(r.c);
    assert!(r.h);
}

#[test]
fn add_a_a_doubles() {
    let r = add8(0x40, 0x40);
    assert_eq!(r.value, 0x80);
    assert!(!r.z);
    assert!(!r.h);
    assert!(!r.c);
}

#[test]
fn bit_zero_when_clear_and_half_carry_set() {
    let f = bit(0b1111_1110, 0);
    assert!(f.z);
    assert!(!f.n);
    assert!(f.h);

    let f = bit(0b0000_0001, 0);
    assert!(!f.z);
    assert!(!f.n);
    assert!(f.h);
}

#[test]
fn rr_shifts_in_carry_and_exports_bit0() {
    let r = rr(0b0000_0001, false);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);
    assert!(!r.n);
    assert!(!r.h);

    let r = rr(0b0000_0000, true);
    assert_eq!(r.value, 0x80);
    assert!(!r.z);
    assert!(!r.c);
}

#[test]
fn rl_rlc_rrc_basics() {
    let r = rl(0x80, false);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);

    let r = rlc(0x80);
    assert_eq!(r.value, 0x01);
    assert!(r.c);

    let r = rrc(0x01);
    assert_eq!(r.value, 0x80);
    assert!(r.c);
}

#[test]
fn sla_sra_srl_shift_semantics() {
    let r = sla(0x80);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);
    assert!(!r.n);
    assert!(!r.h);

    let r = sla(0x40);
    assert_eq!(r.value, 0x80);
    assert!(!r.z);
    assert!(!r.c);

    let r = sra(0x81);
    assert_eq!(r.value, 0xC0); // sign bit preserved
    assert!(!r.z);
    assert!(r.c);
    assert!(!r.n);
    assert!(!r.h);

    let r = sra(0x01);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);

    let r = srl(0x81);
    assert_eq!(r.value, 0x40); // sign bit cleared
    assert!(!r.z);
    assert!(r.c);

    let r = srl(0x01);
    assert_eq!(r.value, 0x00);
    assert!(r.z);
    assert!(r.c);
    assert!(!r.n);
    assert!(!r.h);
}

#[test]
fn add_sp_e8_positive_and_negative_offsets() {
    let r = add_sp_e8(0x1000, 2);
    assert_eq!(r.value, 0x1002);
    assert!(!r.h);
    assert!(!r.c);

    let r = add_sp_e8(0x1000, -1);
    assert_eq!(r.value, 0x0FFF);
    assert!(!r.h);
    assert!(!r.c);
}

#[test]
fn add_sp_e8_half_carry_without_carry() {
    let r = add_sp_e8(0x000F, 1);
    assert_eq!(r.value, 0x0010);
    assert!(r.h);
    assert!(!r.c);
}

#[test]
fn add_sp_e8_carry_and_half_carry() {
    let r = add_sp_e8(0x00FF, 1);
    assert_eq!(r.value, 0x0100);
    assert!(r.h);
    assert!(r.c);
}

#[test]
fn add_sp_e8_negative_offset_flag_patterns() {
    // SP=0x0001, e=-1 → 0x0000; low-byte add 0x01+0xFF carries
    let r = add_sp_e8(0x0001, -1);
    assert_eq!(r.value, 0x0000);
    assert!(r.h);
    assert!(r.c);

    // SP=0x0010, e=-1 → 0x000F; 0x10+0xFF = 0x10F → C, nibble 0+F no H
    let r = add_sp_e8(0x0010, -1);
    assert_eq!(r.value, 0x000F);
    assert!(!r.h);
    assert!(r.c);
}

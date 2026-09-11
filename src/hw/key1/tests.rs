use super::{KEY1, Key1};

#[test]
fn key1_mmio_address_is_ff4d() {
    assert_eq!(KEY1, 0xFF4D);
}

#[test]
fn power_on_read_is_7e() {
    let key1 = Key1::new();
    assert_eq!(key1.read(), 0x7E);
    assert!(!key1.current_double());
    assert!(!key1.armed());
}

#[test]
fn write_one_sets_armed() {
    let mut key1 = Key1::new();
    key1.write(1);
    assert!(key1.armed());
    assert!(!key1.current_double());
    assert_eq!(key1.read(), 0x7F);
}

#[test]
fn write_0x80_does_not_set_speed() {
    let mut key1 = Key1::new();
    key1.write(0x80);
    assert!(!key1.current_double());
    assert!(!key1.armed());
    assert_eq!(key1.read(), 0x7E);
}

#[test]
fn write_ignores_bit_7_and_unused_bits() {
    let mut key1 = Key1::new();
    key1.write(0xFF);
    assert!(key1.armed());
    assert!(!key1.current_double());
    assert_eq!(key1.read(), 0x7F);
}

#[test]
fn try_switch_on_stop_when_not_armed_is_false() {
    let mut key1 = Key1::new();
    assert!(!key1.try_switch_on_stop());
    assert!(!key1.current_double());
    assert!(!key1.armed());
    assert_eq!(key1.read(), 0x7E);
}

#[test]
fn armed_stop_toggles_speed_and_clears_armed() {
    let mut key1 = Key1::new();
    key1.write(1);
    assert!(key1.try_switch_on_stop());
    assert!(!key1.armed());
    assert!(key1.current_double());
    assert_eq!(key1.read(), 0xFE);
}

#[test]
fn second_stop_without_rearm_does_nothing() {
    let mut key1 = Key1::new();
    key1.write(1);
    assert!(key1.try_switch_on_stop());
    assert!(!key1.try_switch_on_stop());
    assert!(key1.current_double());
    assert!(!key1.armed());
    assert_eq!(key1.read(), 0xFE);
}

#[test]
fn rearm_and_stop_returns_to_normal_speed() {
    let mut key1 = Key1::new();
    key1.write(1);
    assert!(key1.try_switch_on_stop());
    key1.write(1);
    assert!(key1.try_switch_on_stop());
    assert!(!key1.current_double());
    assert!(!key1.armed());
    assert_eq!(key1.read(), 0x7E);
}

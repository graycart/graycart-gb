use super::{KEY0, Key0};

#[test]
fn key0_mmio_address_is_ff4c() {
    assert_eq!(KEY0, 0xFF4C);
}

#[test]
fn power_on_is_zero_and_unlocked() {
    let key0 = Key0::new();
    assert_eq!(key0.read(), 0);
    assert!(!key0.is_locked());
}

#[test]
fn write_then_lock_keeps_stored_value() {
    let mut key0 = Key0::new();
    key0.write(0x80);
    assert_eq!(key0.read(), 0x80);
    assert!(!key0.is_locked());

    key0.lock();
    assert!(key0.is_locked());
    assert_eq!(key0.read(), 0x80);
}

#[test]
fn set_and_lock_stores_value_and_locks() {
    let mut key0 = Key0::new();
    key0.set_and_lock(0xC0);
    assert_eq!(key0.read(), 0xC0);
    assert!(key0.is_locked());

    let mut compat = Key0::new();
    compat.set_and_lock(0x04);
    assert_eq!(compat.read(), 0x04);
    assert!(compat.is_locked());
}

#[test]
fn post_lock_write_is_ignored() {
    let mut key0 = Key0::new();
    key0.write(0x80);
    key0.lock();
    key0.write(0x04);
    assert_eq!(key0.read(), 0x80);
    assert!(key0.is_locked());

    let mut fast = Key0::new();
    fast.set_and_lock(0xC0);
    fast.write(0x00);
    assert_eq!(fast.read(), 0xC0);
    assert!(fast.is_locked());
}

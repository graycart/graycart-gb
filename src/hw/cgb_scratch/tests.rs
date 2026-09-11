use super::{CgbScratch, FF72, FF73, FF74, FF75, FF75_RW, FF75_UNUSED_ONES, ff74_locked};

#[test]
fn scratch_mmio_addresses() {
    assert_eq!(FF72, 0xFF72);
    assert_eq!(FF73, 0xFF73);
    assert_eq!(FF74, 0xFF74);
    assert_eq!(FF75, 0xFF75);
}

#[test]
fn power_on_defaults_are_zero_with_ff75_unused_ones() {
    let scratch = CgbScratch::new();
    assert_eq!(scratch.read(FF72), Some(0x00));
    assert_eq!(scratch.read(FF73), Some(0x00));
    assert_eq!(scratch.read(FF74), Some(0x00));
    assert_eq!(scratch.read(FF75), Some(FF75_UNUSED_ONES));
    assert_eq!(FF75_UNUSED_ONES, 0x8F);
}

#[test]
fn ff72_and_ff73_are_fully_read_write() {
    let mut scratch = CgbScratch::new();
    assert!(scratch.write(FF72, 0x00));
    assert_eq!(scratch.read(FF72), Some(0x00));
    assert!(scratch.write(FF72, 0xFF));
    assert_eq!(scratch.read(FF72), Some(0xFF));
    assert!(scratch.write(FF73, 0x5A));
    assert_eq!(scratch.read(FF73), Some(0x5A));
}

#[test]
fn native_ff74_is_fully_read_write() {
    let mut scratch = CgbScratch::new();
    assert!(scratch.write(FF74, 0x00));
    assert_eq!(scratch.read(FF74), Some(0x00));
    assert!(scratch.write(FF74, 0xFF));
    assert_eq!(scratch.read(FF74), Some(0xFF));
}

#[test]
fn ff75_masks_to_bits_4_6_and_unused_read_as_one() {
    let mut scratch = CgbScratch::new();
    assert_eq!(FF75_RW, 0x70);
    assert!(scratch.write(FF75, 0x00));
    assert_eq!(scratch.read(FF75), Some(0x8F));
    assert!(scratch.write(FF75, 0xFF));
    assert_eq!(scratch.read(FF75), Some(0xFF));
    assert!(scratch.write(FF75, 0x0F));
    assert_eq!(scratch.read(FF75), Some(0x8F));
    assert!(scratch.write(FF75, 0x50));
    assert_eq!(scratch.read(FF75), Some(0x50 | 0x8F));
}

#[test]
fn non_cgb_mode_ff74_is_locked_at_ff() {
    assert_eq!(ff74_locked(), 0xFF);
}

#[test]
fn unknown_addr_is_not_scratch() {
    let mut scratch = CgbScratch::new();
    assert_eq!(scratch.read(0xFF71), None);
    assert!(!scratch.write(0xFF71, 0x00));
}

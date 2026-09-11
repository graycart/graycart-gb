use super::*;

#[test]
fn svbk_address_is_ff70() {
    assert_eq!(SVBK, 0xFF70);
}

#[test]
fn c000_always_bank_0_regardless_of_svbk() {
    let mut wram = Wram::new();
    wram.write(0xC000, 0x11, true);
    wram.write_svbk(2);
    wram.write(0xC123, 0x22, true);
    wram.write_svbk(7);
    assert_eq!(wram.read(0xC000, true), 0x11);
    assert_eq!(wram.read(0xC123, true), 0x22);
    wram.write_svbk(0);
    assert_eq!(wram.read(0xC000, true), 0x11);
}

#[test]
fn d000_bank_2_is_isolated_from_bank_1() {
    let mut wram = Wram::new();
    wram.write_svbk(1);
    wram.write(0xD000, 0xAA, true);
    wram.write_svbk(2);
    wram.write(0xD000, 0xBB, true);
    wram.write(0xDFFF, 0xCC, true);

    wram.write_svbk(1);
    assert_eq!(wram.read(0xD000, true), 0xAA);
    assert_eq!(wram.read(0xDFFF, true), 0);

    wram.write_svbk(2);
    assert_eq!(wram.read(0xD000, true), 0xBB);
    assert_eq!(wram.read(0xDFFF, true), 0xCC);
}

#[test]
fn svbk_0_aliases_bank_1() {
    let mut wram = Wram::new();
    wram.write_svbk(1);
    wram.write(0xD010, 0x42, true);
    wram.write_svbk(0);
    assert_eq!(wram.mapped_bank_d(), 1);
    assert_eq!(wram.read(0xD010, true), 0x42);
    wram.write(0xD010, 0x43, true);
    wram.write_svbk(1);
    assert_eq!(wram.read(0xD010, true), 0x43);
    assert_eq!(wram.read_svbk(), 0xF9);
    wram.write_svbk(0);
    assert_eq!(wram.read_svbk(), 0xF9);
}

#[test]
fn dmg_ignores_svbk_and_uses_bank_1() {
    let mut wram = Wram::new();
    wram.write_svbk(1);
    wram.write(0xD000, 0x11, false);
    wram.write_svbk(3);
    wram.write(0xD000, 0x22, false);
    assert_eq!(wram.read(0xD000, false), 0x22);
    wram.write_svbk(1);
    assert_eq!(wram.read(0xD000, false), 0x22);

    wram.write_svbk(3);
    wram.write(0xD000, 0x33, true);
    assert_eq!(wram.read(0xD000, false), 0x22);
    assert_eq!(wram.read(0xD000, true), 0x33);
}

#[test]
fn snapshot_8k_is_c000_then_mapped_d000() {
    let mut wram = Wram::new();
    wram.write_svbk(2);
    wram.write(0xC000, 0xA0, true);
    wram.write(0xCFFF, 0xA1, true);
    wram.write(0xD000, 0xB0, true);
    wram.write(0xDFFF, 0xB1, true);
    wram.write_svbk(1);
    wram.write(0xD000, 0xC0, true);

    wram.write_svbk(2);
    let snap = wram.snapshot_8k(true);
    assert_eq!(snap[0], 0xA0);
    assert_eq!(snap[0x0FFF], 0xA1);
    assert_eq!(snap[0x1000], 0xB0);
    assert_eq!(snap[0x1FFF], 0xB1);

    wram.fill(0xFF);
    assert_eq!(wram.read(0xC000, true), 0xFF);
    wram.apply_snapshot_8k(&snap, true);
    assert_eq!(wram.read(0xC000, true), 0xA0);
    assert_eq!(wram.read(0xD000, true), 0xB0);
    wram.write_svbk(1);
    assert_eq!(wram.read(0xD000, true), 0xFF);
}

#[test]
fn dmg_snapshot_8k_uses_bank_1_window() {
    let mut wram = Wram::new();
    wram.write_svbk(2);
    wram.write(0xC000, 0x10, false);
    wram.write(0xD000, 0x20, false);
    wram.write(0xD000, 0x99, true);

    let snap = wram.snapshot_8k(false);
    assert_eq!(snap[0], 0x10);
    assert_eq!(snap[0x1000], 0x20);
}

#[test]
fn power_on_reset_canonicalizes_svbk_to_bank_1() {
    let mut wram = Wram::new();
    wram.write_svbk(7);
    wram.write(0xD000, 0x77, true);
    wram.power_on_reset();
    assert_eq!(wram.read_svbk(), 0xF9);
    assert_eq!(wram.mapped_bank_d(), 1);
    assert_eq!(wram.read(0xD000, true), 0);
}

#[test]
fn svbk_unused_bits_read_as_one() {
    let mut wram = Wram::new();
    wram.write_svbk(0xFF);
    assert_eq!(wram.read_svbk(), 0xFF);
    assert_eq!(wram.mapped_bank_d(), 7);
    wram.write_svbk(0x00);
    assert_eq!(wram.read_svbk(), 0xF9);
}

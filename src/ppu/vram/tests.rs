use super::{VBK, Vram};

const ADDR: u16 = 0x8000;
const ADDR_END: u16 = 0x9FFF;

#[test]
fn vbk_mmio_address_is_ff4f() {
    assert_eq!(VBK, 0xFF4F);
}

#[test]
fn write_vbk_keeps_bit0_unused_bits_read_as_one() {
    let mut vram = Vram::new();
    vram.write_vbk(0xFF);
    assert_eq!(vram.read_vbk(), 0xFF);
    vram.write_vbk(0x00);
    assert_eq!(vram.read_vbk(), 0xFE);
    vram.write_vbk(0x02);
    assert_eq!(vram.read_vbk(), 0xFE);
}

#[test]
fn cgb_cpu_access_isolates_banks() {
    let mut vram = Vram::new();
    vram.write_vbk(0x00);
    vram.cpu_write(ADDR, 0x11, true);
    vram.cpu_write(ADDR_END, 0x22, true);

    vram.write_vbk(0x01);
    vram.cpu_write(ADDR, 0xAA, true);
    vram.cpu_write(ADDR_END, 0xBB, true);

    assert_eq!(vram.cpu_read(ADDR, true), 0xAA);
    assert_eq!(vram.cpu_read(ADDR_END, true), 0xBB);
    assert_eq!(vram.bank0()[0], 0x11);
    assert_eq!(vram.bank0()[0x1FFF], 0x22);

    vram.write_vbk(0x00);
    assert_eq!(vram.cpu_read(ADDR, true), 0x11);
    assert_eq!(vram.cpu_read(ADDR_END, true), 0x22);
}

#[test]
fn dmg_cpu_access_ignores_vbk_and_uses_bank0() {
    let mut vram = Vram::new();
    vram.write_vbk(0x00);
    vram.cpu_write(ADDR, 0x33, false);

    vram.write_vbk(0x01);
    vram.cpu_write(ADDR, 0x44, false);
    assert_eq!(vram.cpu_read(ADDR, false), 0x44);
    assert_eq!(vram.bank0()[0], 0x44);

    vram.cpu_write(ADDR, 0x55, true);
    assert_eq!(vram.cpu_read(ADDR, false), 0x44);
    assert_eq!(vram.bank0()[0], 0x44);

    vram.write_vbk(0x00);
    assert_eq!(vram.cpu_read(ADDR, true), 0x44);
}

#[test]
fn bank_0_matches_bank0() {
    let mut vram = Vram::new();
    vram.cpu_write(ADDR, 0x11, true);
    assert_eq!(vram.bank(0)[0], vram.bank0()[0]);
    assert_eq!(vram.bank(0)[0], 0x11);
    assert_eq!(vram.bank(0).len(), 0x2000);
}

#[test]
fn bank_1_is_isolated_from_bank_0() {
    let mut vram = Vram::new();
    vram.write_vbk(0x00);
    vram.cpu_write(ADDR, 0x11, true);
    vram.cpu_write(ADDR_END, 0x22, true);

    vram.write_vbk(0x01);
    vram.cpu_write(ADDR, 0xAA, true);
    vram.cpu_write(ADDR_END, 0xBB, true);

    assert_eq!(vram.bank(0)[0], 0x11);
    assert_eq!(vram.bank(0)[0x1FFF], 0x22);
    assert_eq!(vram.bank(1)[0], 0xAA);
    assert_eq!(vram.bank(1)[0x1FFF], 0xBB);
}

#[test]
fn fill_writes_both_banks() {
    let mut vram = Vram::new();
    vram.write_vbk(0x00);
    vram.cpu_write(ADDR, 0x01, true);
    vram.write_vbk(0x01);
    vram.cpu_write(ADDR, 0x02, true);

    vram.fill(0x7E);

    vram.write_vbk(0x00);
    assert_eq!(vram.cpu_read(ADDR, true), 0x7E);
    assert_eq!(vram.cpu_read(ADDR_END, true), 0x7E);
    vram.write_vbk(0x01);
    assert_eq!(vram.cpu_read(ADDR, true), 0x7E);
    assert_eq!(vram.cpu_read(ADDR_END, true), 0x7E);
    assert!(vram.bank0().iter().all(|&b| b == 0x7E));
}

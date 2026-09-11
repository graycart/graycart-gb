use super::{CGB_BOOT_ROM_SIZE, cgb_boot_byte};

fn unique_rom() -> [u8; CGB_BOOT_ROM_SIZE] {
    let mut rom = [0u8; CGB_BOOT_ROM_SIZE];
    for (i, byte) in rom.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    rom
}

#[test]
fn cgb_boot_rom_is_2048_bytes() {
    assert_eq!(CGB_BOOT_ROM_SIZE, 2048);
}

#[test]
fn addr_0000_hits_file_start() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x0000), Some(rom[0x000]));
}

#[test]
fn addr_00ff_hits_end_of_first_page() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x00FF), Some(rom[0x0FF]));
}

#[test]
fn addr_0100_is_cartridge() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x0100), None);
}

#[test]
fn addr_0143_is_cartridge() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x0143), None);
}

#[test]
fn addr_0200_maps_to_rom_0x100() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x0200), Some(rom[0x100]));
}

#[test]
fn addr_08ff_maps_to_rom_0x7ff() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x08FF), Some(rom[0x7FF]));
}

#[test]
fn addr_0900_is_unmapped() {
    let rom = unique_rom();
    assert_eq!(cgb_boot_byte(&rom, 0x0900), None);
}

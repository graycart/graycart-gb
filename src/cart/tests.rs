use super::*;

/// Build a tiny MBC3+RAM image: bank0 marker at $0000, bank2 marker at bank2+$0000.
fn mbc3_test_rom() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10; // bank 0
    rom[2 * ROM_BANK_SIZE] = 0x22; // bank 2
    rom[0x0147] = 0x13; // MBC3+RAM+BATTERY
    rom[0x0148] = 0x01; // 64 KiB / 4 banks
    rom[0x0149] = 0x02; // 8 KiB RAM
    // Logo + checksums left invalid; we only need type/size for banking.
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn mbc3_rom_bank_zero_maps_to_one() {
    let mut cart = mbc3_test_rom();
    // Default bank is 1; put a marker in bank 1.
    cart.rom[ROM_BANK_SIZE] = 0x11;
    assert_eq!(cart.read8(0x4000), 0x11);

    cart.write8(0x2000, 0x00); // select 0 → 1
    assert_eq!(cart.read8(0x4000), 0x11);

    cart.write8(0x2000, 0x02);
    assert_eq!(cart.read8(0x4000), 0x22);
    assert_eq!(cart.read8(0x0000), 0x10); // fixed bank 0
}

#[test]
fn mbc3_ram_requires_enable() {
    let mut cart = mbc3_test_rom();
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xFF);

    cart.write8(0x0000, 0x0A); // enable
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xAB);

    cart.write8(0x0000, 0x00); // disable
    assert_eq!(cart.read8(0xA000), 0xFF);
}

fn mbc1_test_rom() -> Cartridge {
    // 32 banks × 16 KiB = 512 KiB (matches Link's Awakening size class).
    let mut rom = vec![0xFF; 32 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10; // bank 0
    rom[2 * ROM_BANK_SIZE] = 0x22; // bank 2
    rom[0x14 * ROM_BANK_SIZE] = 0xAA; // bank 0x14 (needs upper bits)
    rom[0x0147] = 0x03; // MBC1+RAM+BATTERY
    rom[0x0148] = 0x04; // 512 KiB / 32 banks
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn mbc1_rom_bank_zero_maps_to_one_and_upper_bits() {
    let mut cart = mbc1_test_rom();
    cart.rom[ROM_BANK_SIZE] = 0x11;
    assert_eq!(cart.read8(0x4000), 0x11);

    cart.write8(0x2000, 0x00);
    assert_eq!(cart.read8(0x4000), 0x11);

    cart.write8(0x2000, 0x02);
    assert_eq!(cart.read8(0x4000), 0x22);
    assert_eq!(cart.read8(0x0000), 0x10);

    // Bank 0x14 = upper 01, lower 0x14
    cart.write8(0x2000, 0x14);
    cart.write8(0x4000, 0x00);
    assert_eq!(cart.read8(0x4000), 0xAA);
}

#[test]
fn mbc1_ram_requires_enable() {
    let mut cart = mbc1_test_rom();
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xFF);
    cart.write8(0x0000, 0x0A);
    cart.write8(0xA000, 0xCD);
    assert_eq!(cart.read8(0xA000), 0xCD);
}

fn mbc5_test_rom() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10; // bank 0
    rom[ROM_BANK_SIZE] = 0x11; // bank 1
    rom[2 * ROM_BANK_SIZE] = 0x22; // bank 2
    rom[0x0147] = 0x1B; // MBC5+RAM+BATTERY
    rom[0x0148] = 0x01; // 64 KiB / 4 banks
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn mbc5_allows_rom_bank_zero_and_bit8() {
    let mut cart = mbc5_test_rom();
    assert_eq!(cart.read8(0x4000), 0x11); // default bank 1

    cart.write8(0x2000, 0x00); // bank 0 allowed
    assert_eq!(cart.read8(0x4000), 0x10);

    cart.write8(0x2000, 0x02);
    assert_eq!(cart.read8(0x4000), 0x22);

    // Bit 8 via $3000 — with only 4 banks, bank 0x102 masks to bank 2.
    cart.write8(0x2000, 0x02);
    cart.write8(0x3000, 0x01);
    assert_eq!(cart.switchable_rom_bank(), 2);
    assert_eq!(cart.read8(0x4000), 0x22);
}

#[test]
fn mbc5_ram_requires_enable() {
    let mut cart = mbc5_test_rom();
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xFF);

    cart.write8(0x0000, 0x0A);
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xAB);

    cart.write8(0x0000, 0x00);
    assert_eq!(cart.read8(0xA000), 0xFF);
}

fn mbc2_test_rom() -> Cartridge {
    // 4 banks × 16 KiB = 64 KiB (MBC2 max is 256 KiB / 16 banks).
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0000] = 0x10; // bank 0
    rom[ROM_BANK_SIZE] = 0x11; // bank 1
    rom[2 * ROM_BANK_SIZE] = 0x22; // bank 2
    rom[0x0147] = 0x06; // MBC2+BATTERY
    rom[0x0148] = 0x01; // 64 KiB / 4 banks
    rom[0x0149] = 0x00; // header says no RAM; MBC2 still has 512×4-bit
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn mbc2_rom_bank_zero_maps_to_one_and_bit8_selects_reg() {
    let mut cart = mbc2_test_rom();
    assert_eq!(cart.read8(0x4000), 0x11); // default bank 1

    // ROM bank write: address bit 8 set (e.g. $2100).
    cart.write8(0x2100, 0x00); // 0 → 1
    assert_eq!(cart.read8(0x4000), 0x11);

    cart.write8(0x2100, 0x02);
    assert_eq!(cart.read8(0x4000), 0x22);
    assert_eq!(cart.read8(0x0000), 0x10);
}

#[test]
fn mbc2_ram_is_4bit_with_echo_and_requires_enable() {
    let mut cart = mbc2_test_rom();
    assert_eq!(cart.external_ram().len(), 512);

    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xFF);

    // RAM enable: address bit 8 clear (e.g. $0000).
    cart.write8(0x0000, 0x0A);
    cart.write8(0xA000, 0xAB);
    assert_eq!(cart.read8(0xA000), 0xFB); // low nibble 0xB, high open-bus 0xF
    // Echo at $A200 (same low 9 bits as $A000).
    assert_eq!(cart.read8(0xA200), 0xFB);

    cart.write8(0x0000, 0x00); // disable
    assert_eq!(cart.read8(0xA000), 0xFF);
}

#[test]
fn unsupported_mapper_fails_clearly() {
    let mut rom = vec![0xFF; 0x200];
    rom[0x0147] = 0x22; // MBC7
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    let header = Header::parse(&rom).unwrap();
    match Cartridge::from_parts(rom, header) {
        Err(e) => assert!(e.to_string().contains("unsupported mapper"), "got: {e}"),
        Ok(_) => panic!("MBC7 should be rejected"),
    }
}

fn mbc3_timer_ram_cart() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK_SIZE];
    rom[0x0147] = 0x10; // MBC3+TIMER+RAM+BATTERY
    rom[0x0148] = 0x01;
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn mbc3_rtc_latch_and_read_through_a000() {
    let mut cart = mbc3_timer_ram_cart();
    assert!(cart.has_rtc());

    cart.write8(0x0000, 0x0A); // enable RAM/RTC
    cart.write8(0x4000, 0x08); // select RTC S
    cart.write8(0xA000, 0x2A); // write seconds
    cart.write8(0x6000, 0x00);
    cart.write8(0x6000, 0x01); // latch
    assert_eq!(cart.read8(0xA000), 0x2A);

    cart.write8(0x4000, 0x00); // RAM bank 0
    cart.write8(0xA000, 0x55);
    assert_eq!(cart.read8(0xA000), 0x55);
}

#[test]
fn mbc3_rtc_ticks_with_cart_time() {
    use crate::cart::mbc::rtc::T_CYCLES_PER_SECOND;

    let mut cart = mbc3_timer_ram_cart();
    cart.write8(0x0000, 0x0A);
    cart.write8(0x4000, 0x08);
    cart.write8(0xA000, 0x00);
    cart.tick(T_CYCLES_PER_SECOND as u32);
    cart.write8(0x6000, 0x00);
    cart.write8(0x6000, 0x01);
    assert_eq!(cart.read8(0xA000), 0x01);
}

#[test]
fn mbc3_rtc_save_image_appends_trailer() {
    let mut cart = mbc3_timer_ram_cart();
    cart.write8(0x0000, 0x0A);
    cart.write8(0x4000, 0x08);
    cart.write8(0xA000, 0x11);
    cart.write8(0x6000, 0x00);
    cart.write8(0x6000, 0x01);

    let image = cart.save_image();
    assert_eq!(image.len(), 8 * 1024 + 48);

    let mut cart2 = mbc3_timer_ram_cart();
    cart2.load_save_image(&image);
    cart2.write8(0x0000, 0x0A);
    cart2.write8(0x4000, 0x08);
    // Latched value restored; wall-clock sync may advance seconds slightly.
    let s = cart2.read8(0xA000);
    assert!(s == 0x11 || s == 0x12, "unexpected seconds {s:#x}");
}

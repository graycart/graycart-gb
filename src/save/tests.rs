use super::*;
use crate::cart::{Cartridge, Header};
use std::fs;
use std::path::PathBuf;

const ROM_BANK: usize = 16 * 1024;

fn mbc3_battery_cart() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK];
    rom[0x0147] = 0x13; // MBC3+RAM+BATTERY
    rom[0x0148] = 0x01; // 64 KiB
    rom[0x0149] = 0x02; // 8 KiB RAM
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

fn mbc3_ram_no_battery() -> Cartridge {
    let mut rom = vec![0xFF; 4 * ROM_BANK];
    rom[0x0147] = 0x12; // MBC3+RAM (no battery)
    rom[0x0148] = 0x01;
    rom[0x0149] = 0x02;
    let header = Header::parse(&rom).unwrap();
    Cartridge::from_parts(rom, header).unwrap()
}

#[test]
fn save_path_in_dir_uses_stem_sav() {
    assert_eq!(
        save_path_in_dir("/data/Graycart/saves", "carts/pokemon_red.gb"),
        PathBuf::from("/data/Graycart/saves/pokemon_red.sav")
    );
    assert_eq!(
        save_path_in_dir("/data/Graycart/saves", "game.GBC"),
        PathBuf::from("/data/Graycart/saves/game.sav")
    );
    assert_eq!(
        save_path_in_dir("/data/Graycart/saves", "rom.bin"),
        PathBuf::from("/data/Graycart/saves/rom.sav")
    );
}

#[test]
fn legacy_sidecar_save_path_beside_rom() {
    assert_eq!(
        legacy_sidecar_save_path("carts/pokemon_red.gb"),
        PathBuf::from("carts/pokemon_red.sav")
    );
    assert_eq!(
        legacy_sidecar_save_path("game.GBC"),
        PathBuf::from("game.sav")
    );
    assert_eq!(
        legacy_sidecar_save_path("rom.bin"),
        PathBuf::from("rom.bin.sav")
    );
}

#[test]
fn default_save_path_uses_graycart_saves_dir() {
    let path = default_save_path("carts/pokemon_red.gb");
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("pokemon_red.sav")
    );
    assert_eq!(path.parent().map(PathBuf::from), Some(graycart_saves_dir()));
    assert_ne!(path, PathBuf::from("carts/pokemon_red.sav"));
}

#[test]
fn load_with_fallback_prefers_primary() {
    let dir = tempfile::tempdir().unwrap();
    let primary = dir.path().join("primary.sav");
    let legacy = dir.path().join("legacy.sav");

    let mut writer = mbc3_battery_cart();
    writer.write8(0x0000, 0x0A);
    writer.write8(0xA000, 0x11);
    assert!(flush(&primary, &mut writer).unwrap());

    let mut legacy_writer = mbc3_battery_cart();
    legacy_writer.write8(0x0000, 0x0A);
    legacy_writer.write8(0xA000, 0x22);
    assert!(flush(&legacy, &mut legacy_writer).unwrap());

    let mut cart = mbc3_battery_cart();
    assert!(load_with_fallback(&primary, Some(&legacy), &mut cart).unwrap());
    cart.write8(0x0000, 0x0A);
    assert_eq!(cart.read8(0xA000), 0x11);
}

#[test]
fn load_with_fallback_uses_legacy_when_primary_missing() {
    let dir = tempfile::tempdir().unwrap();
    let primary = dir.path().join("missing.sav");
    let legacy = dir.path().join("legacy.sav");
    let _ = fs::remove_file(&primary);

    let mut writer = mbc3_battery_cart();
    writer.write8(0x0000, 0x0A);
    writer.write8(0xA000, 0x33);
    assert!(flush(&legacy, &mut writer).unwrap());

    let mut cart = mbc3_battery_cart();
    assert!(load_with_fallback(&primary, Some(&legacy), &mut cart).unwrap());
    cart.write8(0x0000, 0x0A);
    assert_eq!(cart.read8(0xA000), 0x33);
}

#[test]
fn load_with_fallback_skips_legacy_when_none() {
    let dir = tempfile::tempdir().unwrap();
    let primary = dir.path().join("missing.sav");
    let legacy = dir.path().join("legacy.sav");
    let _ = fs::remove_file(&primary);

    let mut writer = mbc3_battery_cart();
    writer.write8(0x0000, 0x0A);
    writer.write8(0xA000, 0x44);
    assert!(flush(&legacy, &mut writer).unwrap());

    let mut cart = mbc3_battery_cart();
    assert!(!load_with_fallback(&primary, None, &mut cart).unwrap());
}

#[test]
fn flush_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("saves").join("test.sav");
    assert!(!path.parent().unwrap().exists());

    let mut cart = mbc3_battery_cart();
    cart.write8(0x0000, 0x0A);
    cart.write8(0xA000, 0x55);
    assert!(flush(&path, &mut cart).unwrap());
    assert!(path.is_file());
}

#[test]
fn load_missing_file_is_ok_false() {
    let mut cart = mbc3_battery_cart();
    let path = std::env::temp_dir().join("graycart-missing-save-test.sav");
    let _ = fs::remove_file(&path);
    assert!(!load(&path, &mut cart).unwrap());
}

#[test]
fn flush_and_reload_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sav");

    let mut cart = mbc3_battery_cart();
    cart.write8(0x0000, 0x0A); // enable RAM
    cart.write8(0xA000, 0x42);
    cart.write8(0xA001, 0x99);
    assert!(cart.is_save_dirty());

    assert!(flush(&path, &mut cart).unwrap());
    assert!(!cart.is_save_dirty());
    assert_eq!(fs::metadata(&path).unwrap().len(), 8 * 1024);

    let mut cart2 = mbc3_battery_cart();
    assert!(load(&path, &mut cart2).unwrap());
    cart2.write8(0x0000, 0x0A);
    assert_eq!(cart2.read8(0xA000), 0x42);
    assert_eq!(cart2.read8(0xA001), 0x99);
}

#[test]
fn non_battery_ram_is_not_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sav");

    let mut cart = mbc3_ram_no_battery();
    assert!(!cart.has_battery_backed_ram());
    cart.write8(0x0000, 0x0A);
    cart.write8(0xA000, 0x55);
    assert!(!cart.is_ram_dirty());
    assert!(!cart.is_save_dirty());
    assert!(!flush(&path, &mut cart).unwrap());
    assert!(!path.exists());
}

#[test]
fn mbc3_timer_save_includes_rtc_trailer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sav");

    let mut rom = vec![0xFF; 4 * ROM_BANK];
    rom[0x0147] = 0x10; // MBC3+TIMER+RAM+BATTERY
    rom[0x0148] = 0x01;
    rom[0x0149] = 0x02;
    let header = Header::parse(&rom).unwrap();
    let mut cart = Cartridge::from_parts(rom, header).unwrap();
    assert!(cart.has_rtc());

    cart.write8(0x0000, 0x0A);
    cart.write8(0x4000, 0x08);
    cart.write8(0xA000, 0x07);
    cart.write8(0x6000, 0x00);
    cart.write8(0x6000, 0x01);
    assert!(flush(&path, &mut cart).unwrap());
    assert_eq!(fs::metadata(&path).unwrap().len(), 8 * 1024 + 48);

    let mut cart2 = {
        let mut rom = vec![0xFF; 4 * ROM_BANK];
        rom[0x0147] = 0x10;
        rom[0x0148] = 0x01;
        rom[0x0149] = 0x02;
        let header = Header::parse(&rom).unwrap();
        Cartridge::from_parts(rom, header).unwrap()
    };
    assert!(load(&path, &mut cart2).unwrap());
    cart2.write8(0x0000, 0x0A);
    cart2.write8(0x4000, 0x08);
    let s = cart2.read8(0xA000);
    assert!(s == 0x07 || s == 0x08, "unexpected {s:#x}");
}

#[test]
fn load_truncates_oversized_sav() {
    let mut cart = mbc3_battery_cart();
    let mut data = vec![0xAA; 8 * 1024 + 100];
    data[0] = 0x11;
    cart.load_external_ram(&data);
    assert_eq!(cart.external_ram().len(), 8 * 1024);
    assert_eq!(cart.external_ram()[0], 0x11);
    assert_eq!(cart.external_ram()[1], 0xAA);
}

#[test]
fn load_zero_pads_short_sav() {
    let mut cart = mbc3_battery_cart();
    cart.load_external_ram(&[0x7E, 0x7F]);
    assert_eq!(cart.external_ram()[0], 0x7E);
    assert_eq!(cart.external_ram()[1], 0x7F);
    assert_eq!(cart.external_ram()[2], 0x00);
}

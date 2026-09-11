use super::{
    CGB_SILICON_READY, CgbExecutionMode, CgbSupportClass, HardwareModel, HostHardwarePref,
    LaunchError, bus_from_cartridge, resolve_launch,
};
use crate::cart::{Cartridge, CartridgeCgbSupport};

fn cgb(mode: CgbExecutionMode) -> HardwareModel {
    HardwareModel::Cgb { mode }
}

#[test]
fn cgb_silicon_ready_enables_automatic_cgb_path() {
    const {
        assert!(CGB_SILICON_READY);
    }
}

#[test]
fn host_hardware_pref_defaults_to_automatic() {
    assert_eq!(HostHardwarePref::default(), HostHardwarePref::Automatic);
}

#[test]
fn host_hardware_pref_serde_roundtrip_uses_snake_case() {
    let json = serde_json::to_string(&HostHardwarePref::GameBoyColor).unwrap();
    assert_eq!(json, "\"game_boy_color\"");
    let back: HostHardwarePref = serde_json::from_str(&json).unwrap();
    assert_eq!(back, HostHardwarePref::GameBoyColor);

    let automatic: HostHardwarePref = serde_json::from_str("\"automatic\"").unwrap();
    assert_eq!(automatic, HostHardwarePref::Automatic);
}

#[test]
fn host_hardware_pref_menu_covers_three_choices() {
    assert_eq!(HostHardwarePref::ALL.len(), 3);
    assert_eq!(HostHardwarePref::Automatic.menu_label(), "Automatic");
    assert_eq!(HostHardwarePref::GameBoy.menu_label(), "Game Boy");
    assert_eq!(
        HostHardwarePref::GameBoyColor.menu_label(),
        "Game Boy Color"
    );
}

#[test]
fn hardware_model_launch_labels() {
    assert_eq!(HardwareModel::Dmg.launch_label(), "Game Boy");
    assert_eq!(
        cgb(CgbExecutionMode::NativeCgb).launch_label(),
        "Game Boy Color"
    );
    assert_eq!(
        cgb(CgbExecutionMode::DmgCompatibility).launch_label(),
        "Game Boy Color (DMG compatibility)"
    );
}

#[test]
fn filename_gbc_is_not_a_hardware_selector() {
    // Header support + host pref resolve the model. A `.gbc` suffix never
    // appears in this API and must not be inferred from a CGB-capable class.
    let model =
        resolve_launch(CgbSupportClass::DmgOnly, HostHardwarePref::Automatic, false).unwrap();
    assert_eq!(model, HardwareModel::Dmg);
}

const READY: bool = true;
const PRE_TRUST: bool = false;

#[test]
fn ready_dmg_only_automatic_is_cgb_compat() {
    assert_eq!(
        resolve_launch(CgbSupportClass::DmgOnly, HostHardwarePref::Automatic, READY),
        Ok(cgb(CgbExecutionMode::DmgCompatibility))
    );
}

#[test]
fn ready_dmg_only_game_boy_is_dmg() {
    assert_eq!(
        resolve_launch(CgbSupportClass::DmgOnly, HostHardwarePref::GameBoy, READY),
        Ok(HardwareModel::Dmg)
    );
}

#[test]
fn ready_dmg_only_game_boy_color_is_cgb_compat() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::DmgOnly,
            HostHardwarePref::GameBoyColor,
            READY
        ),
        Ok(cgb(CgbExecutionMode::DmgCompatibility))
    );
}

#[test]
fn ready_cgb_compatible_automatic_is_native_cgb() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::Automatic,
            READY
        ),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
}

#[test]
fn ready_cgb_compatible_game_boy_is_dmg() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::GameBoy,
            READY
        ),
        Ok(HardwareModel::Dmg)
    );
}

#[test]
fn ready_cgb_compatible_game_boy_color_is_native_cgb() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::GameBoyColor,
            READY
        ),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
}

#[test]
fn ready_cgb_only_automatic_is_native_cgb() {
    assert_eq!(
        resolve_launch(CgbSupportClass::CgbOnly, HostHardwarePref::Automatic, READY),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
}

#[test]
fn ready_cgb_only_game_boy_is_rejected() {
    assert_eq!(
        resolve_launch(CgbSupportClass::CgbOnly, HostHardwarePref::GameBoy, READY),
        Err(LaunchError::CgbOnlyOnDmg)
    );
}

#[test]
fn ready_cgb_only_game_boy_color_is_native_cgb() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbOnly,
            HostHardwarePref::GameBoyColor,
            READY
        ),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
}

#[test]
fn pre_trust_automatic_dmg_only_is_dmg() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::DmgOnly,
            HostHardwarePref::Automatic,
            PRE_TRUST
        ),
        Ok(HardwareModel::Dmg)
    );
}

#[test]
fn pre_trust_automatic_cgb_compatible_is_dmg() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::Automatic,
            PRE_TRUST
        ),
        Ok(HardwareModel::Dmg)
    );
}

#[test]
fn pre_trust_automatic_cgb_only_is_rejected() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbOnly,
            HostHardwarePref::Automatic,
            PRE_TRUST
        ),
        Err(LaunchError::CgbOnlyOnDmg)
    );
}

#[test]
fn pre_trust_force_game_boy_follows_force_column() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::DmgOnly,
            HostHardwarePref::GameBoy,
            PRE_TRUST
        ),
        Ok(HardwareModel::Dmg)
    );
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::GameBoy,
            PRE_TRUST
        ),
        Ok(HardwareModel::Dmg)
    );
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbOnly,
            HostHardwarePref::GameBoy,
            PRE_TRUST
        ),
        Err(LaunchError::CgbOnlyOnDmg)
    );
}

#[test]
fn pre_trust_force_game_boy_color_follows_force_column() {
    assert_eq!(
        resolve_launch(
            CgbSupportClass::DmgOnly,
            HostHardwarePref::GameBoyColor,
            PRE_TRUST
        ),
        Ok(cgb(CgbExecutionMode::DmgCompatibility))
    );
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbCompatible,
            HostHardwarePref::GameBoyColor,
            PRE_TRUST
        ),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
    assert_eq!(
        resolve_launch(
            CgbSupportClass::CgbOnly,
            HostHardwarePref::GameBoyColor,
            PRE_TRUST
        ),
        Ok(cgb(CgbExecutionMode::NativeCgb))
    );
}

#[test]
fn from_cartridge_support_maps_header_classes() {
    assert_eq!(
        CgbSupportClass::from_cartridge_support(CartridgeCgbSupport::DmgOnly),
        CgbSupportClass::DmgOnly
    );
    assert_eq!(
        CgbSupportClass::from_cartridge_support(CartridgeCgbSupport::CgbCompatible),
        CgbSupportClass::CgbCompatible
    );
    assert_eq!(
        CgbSupportClass::from_cartridge_support(CartridgeCgbSupport::CgbOnly),
        CgbSupportClass::CgbOnly
    );
    assert_eq!(
        CgbSupportClass::from_cartridge_support(CartridgeCgbSupport::Other(0x81)),
        CgbSupportClass::CgbCompatible
    );
    assert_eq!(
        CgbSupportClass::from_cartridge_support(CartridgeCgbSupport::Other(0x01)),
        CgbSupportClass::DmgOnly
    );
}

fn rom_only_with_cgb_byte(cgb_byte: u8) -> Cartridge {
    let mut rom = vec![0u8; 0x200];
    rom[0x0143] = cgb_byte;
    Cartridge::rom_only(rom)
}

#[test]
fn bus_from_cartridge_dmg_only_automatic_is_cgb_compat() {
    let cart = rom_only_with_cgb_byte(0x00);
    assert_eq!(cart.header.cgb, CartridgeCgbSupport::DmgOnly);
    let bus = bus_from_cartridge(cart, HostHardwarePref::Automatic).unwrap();
    assert_eq!(
        bus.hardware_model(),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::DmgCompatibility
        }
    );
}

#[test]
fn bus_from_cartridge_cgb_compatible_force_gbc_is_native() {
    let cart = rom_only_with_cgb_byte(0x80);
    assert_eq!(cart.header.cgb, CartridgeCgbSupport::CgbCompatible);
    let bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).unwrap();
    assert_eq!(
        bus.hardware_model(),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::NativeCgb
        }
    );
}

#[test]
fn bus_from_cartridge_cgb_only_game_boy_is_err() {
    let cart = rom_only_with_cgb_byte(0xC0);
    assert_eq!(cart.header.cgb, CartridgeCgbSupport::CgbOnly);
    let err = match bus_from_cartridge(cart, HostHardwarePref::GameBoy) {
        Err(e) => e,
        Ok(_) => panic!("expected CgbOnlyOnDmg"),
    };
    assert_eq!(err, LaunchError::CgbOnlyOnDmg);
    assert_eq!(err.to_string(), "CGB-only cartridge cannot run on Game Boy");
}

#[test]
fn bus_from_cartridge_uses_header_not_filename() {
    // API takes Cartridge + pref only — no path. A `.gbc` name cannot leak in.
    let cart = rom_only_with_cgb_byte(0x00);
    let bus = bus_from_cartridge(cart, HostHardwarePref::Automatic).unwrap();
    assert_eq!(
        bus.hardware_model(),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::DmgCompatibility
        }
    );
}

#[test]
fn bus_from_cartridge_cgb_only_automatic_is_native() {
    let cart = rom_only_with_cgb_byte(0xC0);
    let bus = bus_from_cartridge(cart, HostHardwarePref::Automatic).unwrap();
    assert_eq!(
        bus.hardware_model(),
        HardwareModel::Cgb {
            mode: CgbExecutionMode::NativeCgb
        }
    );
}

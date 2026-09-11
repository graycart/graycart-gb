use super::CartridgeCgbSupport;

#[test]
fn from_byte_dmg_only_when_bit7_clear() {
    assert_eq!(
        CartridgeCgbSupport::from_byte(0x00),
        CartridgeCgbSupport::DmgOnly
    );
    assert_eq!(
        CartridgeCgbSupport::from_byte(0x7F),
        CartridgeCgbSupport::DmgOnly
    );
}

#[test]
fn from_byte_cgb_compatible_and_only() {
    assert_eq!(
        CartridgeCgbSupport::from_byte(0x80),
        CartridgeCgbSupport::CgbCompatible
    );
    assert_eq!(
        CartridgeCgbSupport::from_byte(0xC0),
        CartridgeCgbSupport::CgbOnly
    );
}

#[test]
fn from_byte_other_homebrew_keeps_bit7() {
    assert_eq!(
        CartridgeCgbSupport::from_byte(0x81),
        CartridgeCgbSupport::Other(0x81)
    );
    assert!(CartridgeCgbSupport::from_byte(0x81).is_cgb());
    assert!(!CartridgeCgbSupport::from_byte(0x00).is_cgb());
}

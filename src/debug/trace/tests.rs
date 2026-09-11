use super::*;

#[test]
fn format_bytes_joins_hex() {
    assert_eq!(format_bytes(&[0xCB, 0xAE, 0x0E]), "CB AE 0E");
    assert_eq!(format_bytes(&[]), "");
}

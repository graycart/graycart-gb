use super::*;

#[test]
fn intersects_and_row_with_flip() {
    let sp = Sprite {
        y: 16, // screen y = 0
        x: 8,
        tile: 0,
        attrs: 0,
    };
    assert!(sp.intersects_line(0, 8));
    assert!(!sp.intersects_line(8, 8));
    assert_eq!(sp.row_in_sprite(0, 8), 0);
    assert_eq!(sp.row_in_sprite(7, 8), 7);

    let flipped = Sprite { attrs: 0x40, ..sp };
    assert_eq!(flipped.row_in_sprite(0, 8), 7);
    assert_eq!(flipped.row_in_sprite(7, 8), 0);
}

#[test]
fn oam_read_write_round_trip() {
    let mut oam = Oam::new();
    assert!(oam.write(0xFE04, 0xAB));
    assert_eq!(oam.read(0xFE04), Some(0xAB));
    assert_eq!(oam.sprite(1).unwrap().y, 0xAB);
}

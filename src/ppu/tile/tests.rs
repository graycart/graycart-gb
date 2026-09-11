use super::*;

#[test]
fn pan_docs_first_row_3c_7e() {
    // From Pan Docs: $3C $7E → color IDs 0 2 3 3 3 3 2 0
    assert_eq!(decode_tile_row(0x3C, 0x7E), [0, 2, 3, 3, 3, 3, 2, 0]);
}

#[test]
fn decode_tile_row_matches_known_bitplane_row() {
    // bit 7 leftmost; hi supplies bit 1, lo bit 0
    let lo = 0x3C; // 0011_1100
    let hi = 0x42; // 0100_0010
    assert_eq!(decode_tile_row(lo, hi), [0, 2, 1, 1, 1, 1, 2, 0]);
}

#[test]
fn all_zero_and_all_three() {
    assert_eq!(decode_tile_row(0x00, 0x00), [0; 8]);
    assert_eq!(decode_tile_row(0xFF, 0xFF), [3; 8]);
}

#[test]
fn low_plane_only_gives_ones() {
    assert_eq!(decode_tile_row(0xFF, 0x00), [1; 8]);
}

#[test]
fn high_plane_only_gives_twos() {
    assert_eq!(decode_tile_row(0x00, 0xFF), [2; 8]);
}

#[test]
fn leftmost_and_rightmost_bits() {
    // Only leftmost pixel set in both planes → color 3 at index 0
    assert_eq!(decode_tile_row(0x80, 0x80)[0], 3);
    assert_eq!(decode_tile_row(0x80, 0x80)[7], 0);
    // Only rightmost pixel
    assert_eq!(decode_tile_row(0x01, 0x01)[7], 3);
    assert_eq!(decode_tile_row(0x01, 0x01)[0], 0);
}

#[test]
fn addressing_8000_and_8800() {
    assert_eq!(tile_addr_8000(0), 0x8000);
    assert_eq!(tile_addr_8000(1), 0x8010);
    assert_eq!(tile_addr_8000(128), 0x8800);

    assert_eq!(tile_addr_8800(0), 0x9000);
    assert_eq!(tile_addr_8800(1), 0x9010);
    assert_eq!(tile_addr_8800(128), 0x8800); // -128 as i8
    assert_eq!(tile_addr_8800(255), 0x8FF0); // -1 as i8
}

#[test]
fn tile_row_offsets() {
    assert_eq!(tile_row_offset(0), 0);
    assert_eq!(tile_row_offset(1), 2);
    assert_eq!(tile_row_offset(7), 14);
}

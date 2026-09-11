use super::{
    HDMA1, HDMA2, HDMA3, HDMA4, HDMA5, Hdma5Write, mask_dest, mask_source, parse_hdma5_write,
    read_hdma5, source_is_vram,
};

#[test]
fn hdma_mmio_addresses() {
    assert_eq!(HDMA1, 0xFF51);
    assert_eq!(HDMA2, 0xFF52);
    assert_eq!(HDMA3, 0xFF53);
    assert_eq!(HDMA4, 0xFF54);
    assert_eq!(HDMA5, 0xFF55);
}

#[test]
fn mask_source_clears_low_nibble() {
    assert_eq!(mask_source(0x0000), 0x0000);
    assert_eq!(mask_source(0x000F), 0x0000);
    assert_eq!(mask_source(0x1234), 0x1230);
    assert_eq!(mask_source(0x7FF0), 0x7FF0);
    assert_eq!(mask_source(0x7FFF), 0x7FF0);
    assert_eq!(mask_source(0xA00C), 0xA000);
    assert_eq!(mask_source(0xDFF7), 0xDFF0);
    assert_eq!(mask_source(0xFFFF), 0xFFF0);
}

#[test]
fn mask_dest_forces_vram_8000_through_9ff0() {
    assert_eq!(mask_dest(0x0004), 0x8000);
    assert_eq!(mask_dest(0x0000), 0x8000);
    assert_eq!(mask_dest(0x1FF0), 0x9FF0);
    assert_eq!(mask_dest(0x8000), 0x8000);
    assert_eq!(mask_dest(0x9FF0), 0x9FF0);
    assert_eq!(mask_dest(0x9FFF), 0x9FF0);
    assert_eq!(mask_dest(0xFFFF), 0x9FF0);
    assert_eq!(mask_dest(0xE004), 0x8000);
    assert_eq!(mask_dest(0x8123), 0x8120);
}

#[test]
fn source_is_vram_after_mask() {
    assert!(source_is_vram(0x8000));
    assert!(source_is_vram(0x9FF0));
    assert!(source_is_vram(0x800F));
    assert!(source_is_vram(0x9FFF));
    assert!(!source_is_vram(0x0000));
    assert!(!source_is_vram(0x7FF0));
    assert!(!source_is_vram(0xA000));
    assert!(!source_is_vram(0xC000));
    assert!(!source_is_vram(0xDFF0));
}

#[test]
fn parse_hdma5_write_gdma_length() {
    assert_eq!(
        parse_hdma5_write(0x00),
        Hdma5Write {
            blocks_minus_1: 0x00,
            hblank: false,
        }
    );
    assert_eq!(
        parse_hdma5_write(0x7F),
        Hdma5Write {
            blocks_minus_1: 0x7F,
            hblank: false,
        }
    );
}

#[test]
fn parse_hdma5_write_hblank_mode() {
    assert_eq!(
        parse_hdma5_write(0x80),
        Hdma5Write {
            blocks_minus_1: 0x00,
            hblank: true,
        }
    );
    assert_eq!(
        parse_hdma5_write(0xFF),
        Hdma5Write {
            blocks_minus_1: 0x7F,
            hblank: true,
        }
    );
}

#[test]
fn read_hdma5_complete_idle_is_ff() {
    assert_eq!(read_hdma5(0xFF, false), 0xFF);
}

#[test]
fn read_hdma5_active_clears_bit7() {
    assert_eq!(read_hdma5(0x00, true), 0x00);
    assert_eq!(read_hdma5(0x7F, true), 0x7F);
    assert_eq!(read_hdma5(0x10, true), 0x10);
}

#[test]
fn read_hdma5_abort_sets_bit7() {
    assert_eq!(read_hdma5(0x00, false), 0x80);
    assert_eq!(read_hdma5(0x0A, false), 0x8A);
    assert_eq!(read_hdma5(0x7F, false), 0xFF);
}

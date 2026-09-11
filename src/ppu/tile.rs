//! Game Boy 2bpp tile helpers ([Pan Docs](https://gbdev.io/pandocs/Tile_Data.html)).

/// Decode one tile row from its low and high bitplane bytes into 8 color IDs (0–3).
///
/// Bit 7 is the leftmost pixel. For each pixel, the high byte supplies bit 1 and the
/// low byte supplies bit 0 of the color ID.
pub fn decode_tile_row(lo: u8, hi: u8) -> [u8; 8] {
    let mut pixels = [0u8; 8];
    for (i, px) in pixels.iter_mut().enumerate() {
        let bit = 7 - i;
        let lsb = (lo >> bit) & 1;
        let msb = (hi >> bit) & 1;
        *px = (msb << 1) | lsb;
    }
    pixels
}

/// Byte offset of row `row` (0–7) within a 16-byte tile.
pub fn tile_row_offset(row: u8) -> usize {
    (row as usize & 7) * 2
}

/// Read one byte from the 8 KiB VRAM slice (`$8000`–`$9FFF`).
pub(crate) fn vram_byte(vram: &[u8], addr: u16) -> u8 {
    let Some(offset) = addr.checked_sub(0x8000) else {
        return 0xFF;
    };
    vram.get(offset as usize).copied().unwrap_or(0xFF)
}

/// `$8000` addressing: unsigned tile id → VRAM address of the tile’s first byte.
pub fn tile_addr_8000(tile_id: u8) -> u16 {
    0x8000 + u16::from(tile_id) * 16
}

/// `$8800` / signed addressing: tile id as `i8`, base `$9000`.
pub fn tile_addr_8800(tile_id: u8) -> u16 {
    let id = i16::from(tile_id as i8);
    (0x9000i32 + i32::from(id) * 16) as u16
}

#[cfg(test)]
mod tests;

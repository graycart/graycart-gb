//! CGB boot ROM split map (Pan Docs Power-Up Sequence).
//! 2048-byte image: 256 + 1792. Not a linear overlay of `$0000–$07FF`.

pub const CGB_BOOT_ROM_SIZE: usize = 2048;

/// Maps a CPU address onto the 2048-byte CGB boot image.
///
/// `$0000–$00FF` is file `[0x000..0x100]`. `$0100–$01FF` is cartridge (header).
/// `$0200–$08FF` is file `[0x100..0x800]` (`offset = addr - $0100`).
pub fn cgb_boot_byte(rom: &[u8; CGB_BOOT_ROM_SIZE], addr: u16) -> Option<u8> {
    match addr {
        0x0000..=0x00FF => Some(rom[addr as usize]),
        0x0100..=0x01FF => None,
        0x0200..=0x08FF => Some(rom[(addr - 0x0100) as usize]),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

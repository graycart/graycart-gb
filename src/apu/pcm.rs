//! CGB PCM12 / PCM34 (`$FF76` / `$FF77`) — digital generator nibbles.
//!
//! [Pan Docs — Audio details](https://gbdev.io/pandocs/Audio_details.html).
//! `Apu::read` must not claim these addresses. Bus: Native CGB → live
//! [`Apu::pcm12`]/[`Apu::pcm34`]; DmgCompatibility → `$00` (CGB Mode only);
//! DMG silicon → unmapped `$FF`.

pub const PCM12: u16 = 0xFF76;
pub const PCM34: u16 = 0xFF77;

pub fn pack_pcm(lo: u8, hi: u8) -> u8 {
    (lo & 0x0F) | ((hi & 0x0F) << 4)
}

#[cfg(test)]
mod tests;

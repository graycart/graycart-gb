//! DMG unused / unmapped `$FFxx` I/O open-bus behavior (Mooneye unused_hwio-GS).
//!
//! `$FF68`–`$FF6B` stay in this set for DMG. CGB silicon handles those ports
//! in [`crate::bus::Bus`] before calling [`is_unmapped`].

/// Unmapped I/O that always reads `$FF` (writes ignored).
pub(super) fn is_unmapped(addr: u16) -> bool {
    matches!(
        addr,
        0xFF03
            | 0xFF08..=0xFF0E
            | 0xFF15
            | 0xFF1F
            | 0xFF27..=0xFF2F
            | 0xFF4C..=0xFF4E
            | 0xFF51..=0xFF6F
            | 0xFF71..=0xFF7F
    )
}

/// Apply unused-bit open bus (read-as-1) for mapped I/O stubs.
pub(super) fn apply_unused_bits(addr: u16, value: u8) -> u8 {
    value
        | match addr {
            0xFF00 => 0xC0, // P1 bits 6–7
            0xFF02 => 0x7E, // SC bits 1–6
            0xFF0F => 0xE0, // IF bits 5–7
            0xFF10 => 0x80, // NR10 bit 7
            0xFF1A => 0x7F, // NR30 bits 0–6
            0xFF1C => 0x9F, // NR32 bits 0–4, 7
            0xFF20 => 0xC0, // NR41 bits 6–7
            0xFF23 => 0x3F, // NR44 bits 0–5
            0xFF26 => 0x70, // NR52 bits 4–6
            _ => 0x00,
        }
}

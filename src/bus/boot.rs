//! Boot firmware overlay mapped over cartridge reads until `$FF50`.
//!
//! Owns the optional firmware image and whether the overlay is currently mapped.
//! Fast-boot paths never install this; snapshot restore clears it via [`BootRom::clear`].

use crate::hw::{CGB_BOOT_ROM_SIZE, cgb_boot_byte};

/// Installable boot firmware. CGB is a split map, not a longer DMG overlay.
enum BootFirmware {
    Dmg(Box<[u8; 256]>),
    Cgb(Box<[u8; CGB_BOOT_ROM_SIZE]>),
}

/// Boot ROM overlay state for [`super::Bus`].
#[derive(Default)]
pub(super) struct BootRom {
    firmware: Option<BootFirmware>,
    mapped: bool,
}

impl BootRom {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Map a 256-byte DMG boot ROM over `$0000`–`$00FF` until `$FF50` or [`Self::clear`].
    pub(super) fn enable_dmg(&mut self, rom: &[u8; 256]) {
        self.firmware = Some(BootFirmware::Dmg(Box::new(*rom)));
        self.mapped = true;
    }

    /// Map 2048-byte CGB firmware (header window `$0100–$01FF` stays cartridge).
    pub(super) fn enable_cgb(&mut self, rom: &[u8; CGB_BOOT_ROM_SIZE]) {
        self.firmware = Some(BootFirmware::Cgb(Box::new(*rom)));
        self.mapped = true;
    }

    /// Force the overlay off and drop firmware (restore / snapshot / power-on).
    pub(super) fn clear(&mut self) {
        self.firmware = None;
        self.mapped = false;
    }

    /// Unmap without dropping firmware (`$FF50` write with non-zero value).
    pub(super) fn unmap(&mut self) {
        self.mapped = false;
    }

    /// True while the boot ROM overlay is mapped for CPU reads.
    pub(super) fn is_mapped(&self) -> bool {
        self.mapped
    }

    /// Byte from the active overlay, if any, for `addr`.
    pub(super) fn overlay_byte(&self, addr: u16) -> Option<u8> {
        if !self.mapped {
            return None;
        }
        match self.firmware.as_ref()? {
            BootFirmware::Dmg(rom) if addr <= 0x00FF => Some(rom[addr as usize]),
            BootFirmware::Cgb(rom) => cgb_boot_byte(rom, addr),
            _ => None,
        }
    }
}

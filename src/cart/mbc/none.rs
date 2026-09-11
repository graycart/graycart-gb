//! ROM-only / no mapper: flat `$0000–$7FFF`, no external RAM window.

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoMbc;

impl NoMbc {
    pub(crate) fn switchable_rom_bank(self) -> u8 {
        1
    }

    pub(crate) fn active_ram_bank(self) -> Option<u8> {
        None
    }

    pub(crate) fn write_reg(self, _addr: u16, _value: u8) {}
}

//! MBC3: ROM/RAM banking + RTC.
//!
//! [Pan Docs — MBC3](https://gbdev.io/pandocs/MBC3.html)

use super::rtc::Rtc;

#[derive(Debug, Clone)]
pub(crate) struct Mbc3 {
    ram_enabled: bool,
    /// Bank mapped at `$4000–$7FFF`. Never `0` (hardware maps that to bank 1).
    rom_bank: u8,
    /// `$00–$07` = RAM bank; `$08–$0C` = RTC register select.
    ram_bank_or_rtc: u8,
    pub(crate) rtc: Rtc,
}

impl Mbc3 {
    pub(crate) fn new() -> Self {
        Self {
            ram_enabled: false,
            rom_bank: 1,
            ram_bank_or_rtc: 0,
            rtc: Rtc::new(),
        }
    }

    pub(crate) fn switchable_rom_bank(&self) -> u8 {
        self.rom_bank
    }

    /// Active external RAM bank, if RAM (not RTC) is selected and enabled.
    pub(crate) fn active_ram_bank(&self) -> Option<u8> {
        if !self.ram_enabled {
            return None;
        }
        match self.ram_bank_or_rtc {
            0x00..=0x07 => Some(self.ram_bank_or_rtc),
            _ => None,
        }
    }

    /// Selected RTC register (`$08`–`$0C`) when RAM/timer is enabled.
    pub(crate) fn selected_rtc_reg(&self) -> Option<u8> {
        if !self.ram_enabled {
            return None;
        }
        match self.ram_bank_or_rtc {
            r @ 0x08..=0x0C => Some(r),
            _ => None,
        }
    }

    pub(crate) fn read_rtc(&self) -> Option<u8> {
        self.selected_rtc_reg().map(|r| self.rtc.read_latched(r))
    }

    pub(crate) fn write_rtc(&mut self, value: u8) -> bool {
        let Some(r) = self.selected_rtc_reg() else {
            return false;
        };
        self.rtc.write_reg(r, value);
        true
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) {
        self.rtc.tick(t_cycles);
    }

    pub(crate) fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                let bank = value & 0x7F;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => {
                self.ram_bank_or_rtc = value & 0x0F;
            }
            0x6000..=0x7FFF => {
                self.rtc.write_latch(value);
            }
            _ => {}
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::Mbc3StateV1 {
        crate::snapshot::Mbc3StateV1 {
            rom_bank: self.rom_bank,
            ram_bank: self.ram_bank_or_rtc,
            ram_enable: self.ram_enabled,
            rtc_reg: self.ram_bank_or_rtc,
            rtc: self.rtc.snapshot(),
        }
    }

    pub(crate) fn apply(&mut self, state: &crate::snapshot::Mbc3StateV1) {
        self.rom_bank = state.rom_bank;
        self.ram_bank_or_rtc = state.ram_bank;
        self.ram_enabled = state.ram_enable;
        self.rtc.apply(&state.rtc);
    }
}

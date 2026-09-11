/// Cartridge type byte at header offset 0x0147 (Pan Docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CartType {
    RomOnly = 0x00,
    Mbc1 = 0x01,
    Mbc1Ram = 0x02,
    Mbc1RamBattery = 0x03,
    Mbc2 = 0x05,
    Mbc2Battery = 0x06,
    RomRam = 0x08,
    RomRamBattery = 0x09,
    Mmm01 = 0x0B,
    Mmm01Ram = 0x0C,
    Mmm01RamBattery = 0x0D,
    Mbc3TimerBattery = 0x0F,
    Mbc3TimerRamBattery = 0x10,
    Mbc3 = 0x11,
    Mbc3Ram = 0x12,
    Mbc3RamBattery = 0x13,
    Mbc5 = 0x19,
    Mbc5Ram = 0x1A,
    Mbc5RamBattery = 0x1B,
    Mbc5Rumble = 0x1C,
    Mbc5RumbleRam = 0x1D,
    Mbc5RumbleRamBattery = 0x1E,
    Mbc6 = 0x20,
    Mbc7SensorRumbleRamBattery = 0x22,
    PocketCamera = 0xFC,
    BandaiTama5 = 0xFD,
    Huc3 = 0xFE,
    Huc1RamBattery = 0xFF,
}

impl CartType {
    /// Parse a known cartridge type byte, if we have a variant for it.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::RomOnly),
            0x01 => Some(Self::Mbc1),
            0x02 => Some(Self::Mbc1Ram),
            0x03 => Some(Self::Mbc1RamBattery),
            0x05 => Some(Self::Mbc2),
            0x06 => Some(Self::Mbc2Battery),
            0x08 => Some(Self::RomRam),
            0x09 => Some(Self::RomRamBattery),
            0x0B => Some(Self::Mmm01),
            0x0C => Some(Self::Mmm01Ram),
            0x0D => Some(Self::Mmm01RamBattery),
            0x0F => Some(Self::Mbc3TimerBattery),
            0x10 => Some(Self::Mbc3TimerRamBattery),
            0x11 => Some(Self::Mbc3),
            0x12 => Some(Self::Mbc3Ram),
            0x13 => Some(Self::Mbc3RamBattery),
            0x19 => Some(Self::Mbc5),
            0x1A => Some(Self::Mbc5Ram),
            0x1B => Some(Self::Mbc5RamBattery),
            0x1C => Some(Self::Mbc5Rumble),
            0x1D => Some(Self::Mbc5RumbleRam),
            0x1E => Some(Self::Mbc5RumbleRamBattery),
            0x20 => Some(Self::Mbc6),
            0x22 => Some(Self::Mbc7SensorRumbleRamBattery),
            0xFC => Some(Self::PocketCamera),
            0xFD => Some(Self::BandaiTama5),
            0xFE => Some(Self::Huc3),
            0xFF => Some(Self::Huc1RamBattery),
            _ => None,
        }
    }

    pub fn is_mbc1(self) -> bool {
        matches!(self, Self::Mbc1 | Self::Mbc1Ram | Self::Mbc1RamBattery)
    }

    pub fn is_mbc2(self) -> bool {
        matches!(self, Self::Mbc2 | Self::Mbc2Battery)
    }

    /// ROM-only style carts (no MBC chip; optional external RAM still via header).
    pub fn is_rom_mapper(self) -> bool {
        matches!(self, Self::RomOnly | Self::RomRam | Self::RomRamBattery)
    }

    /// Whether this cart type has a working mapper implementation.
    pub fn is_supported(self) -> bool {
        self.is_rom_mapper() || self.is_mbc1() || self.is_mbc2() || self.is_mbc3() || self.is_mbc5()
    }

    pub fn is_mbc3(self) -> bool {
        matches!(
            self,
            Self::Mbc3
                | Self::Mbc3Ram
                | Self::Mbc3RamBattery
                | Self::Mbc3TimerBattery
                | Self::Mbc3TimerRamBattery
        )
    }

    pub fn is_mbc5(self) -> bool {
        matches!(
            self,
            Self::Mbc5
                | Self::Mbc5Ram
                | Self::Mbc5RamBattery
                | Self::Mbc5Rumble
                | Self::Mbc5RumbleRam
                | Self::Mbc5RumbleRamBattery
        )
    }

    /// Cartridge types with a battery (SRAM and/or RTC).
    pub fn has_battery(self) -> bool {
        matches!(
            self,
            Self::Mbc1RamBattery
                | Self::Mbc2Battery
                | Self::RomRamBattery
                | Self::Mmm01RamBattery
                | Self::Mbc3TimerBattery
                | Self::Mbc3TimerRamBattery
                | Self::Mbc3RamBattery
                | Self::Mbc5RamBattery
                | Self::Mbc5RumbleRamBattery
                | Self::Mbc7SensorRumbleRamBattery
                | Self::Huc1RamBattery
        )
    }

    /// MBC3 variants that include the real-time clock.
    pub fn has_rtc(self) -> bool {
        matches!(self, Self::Mbc3TimerBattery | Self::Mbc3TimerRamBattery)
    }

    /// Human-readable name for this cartridge type.
    pub fn name(self) -> &'static str {
        match self {
            Self::RomOnly => "ROM ONLY",
            Self::Mbc1 => "MBC1",
            Self::Mbc1Ram => "MBC1+RAM",
            Self::Mbc1RamBattery => "MBC1+RAM+BATTERY",
            Self::Mbc2 => "MBC2",
            Self::Mbc2Battery => "MBC2+BATTERY",
            Self::RomRam => "ROM+RAM",
            Self::RomRamBattery => "ROM+RAM+BATTERY",
            Self::Mmm01 => "MMM01",
            Self::Mmm01Ram => "MMM01+RAM",
            Self::Mmm01RamBattery => "MMM01+RAM+BATTERY",
            Self::Mbc3TimerBattery => "MBC3+TIMER+BATTERY",
            Self::Mbc3TimerRamBattery => "MBC3+TIMER+RAM+BATTERY",
            Self::Mbc3 => "MBC3",
            Self::Mbc3Ram => "MBC3+RAM",
            Self::Mbc3RamBattery => "MBC3+RAM+BATTERY",
            Self::Mbc5 => "MBC5",
            Self::Mbc5Ram => "MBC5+RAM",
            Self::Mbc5RamBattery => "MBC5+RAM+BATTERY",
            Self::Mbc5Rumble => "MBC5+RUMBLE",
            Self::Mbc5RumbleRam => "MBC5+RUMBLE+RAM",
            Self::Mbc5RumbleRamBattery => "MBC5+RUMBLE+RAM+BATTERY",
            Self::Mbc6 => "MBC6",
            Self::Mbc7SensorRumbleRamBattery => "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
            Self::PocketCamera => "POCKET CAMERA",
            Self::BandaiTama5 => "BANDAI TAMA5",
            Self::Huc3 => "HuC3",
            Self::Huc1RamBattery => "HuC1+RAM+BATTERY",
        }
    }
}

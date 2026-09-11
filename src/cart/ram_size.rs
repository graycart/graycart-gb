/// RAM size code at header offset 0x0149 (Pan Docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RamSize {
    /// No RAM
    None = 0x00,
    /// Unused / mistaken homebrew tag; treat as no cartridge RAM
    Unused = 0x01,
    /// 8 KiB, 1 bank
    Kb8 = 0x02,
    /// 32 KiB, 4 banks of 8 KiB each
    Kb32 = 0x03,
    /// 128 KiB, 16 banks of 8 KiB each
    Kb128 = 0x04,
    /// 64 KiB, 8 banks of 8 KiB each
    Kb64 = 0x05,
}

impl RamSize {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::None),
            0x01 => Some(Self::Unused),
            0x02 => Some(Self::Kb8),
            0x03 => Some(Self::Kb32),
            0x04 => Some(Self::Kb128),
            0x05 => Some(Self::Kb64),
            _ => None,
        }
    }

    /// Human-readable SRAM size (e.g. `"8 KiB"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::Unused => "unused",
            Self::Kb8 => "8 KiB",
            Self::Kb32 => "32 KiB",
            Self::Kb128 => "128 KiB",
            Self::Kb64 => "64 KiB",
        }
    }

    /// Number of 8 KiB RAM banks (`0` when none / unused).
    pub fn bank_count(self) -> u8 {
        match self {
            Self::None | Self::Unused => 0,
            Self::Kb8 => 1,
            Self::Kb32 => 4,
            Self::Kb128 => 16,
            Self::Kb64 => 8,
        }
    }

    /// Total external cartridge RAM size in bytes (`0` when none / unused).
    pub fn size_bytes(self) -> usize {
        usize::from(self.bank_count()) * 8 * 1024
    }
}

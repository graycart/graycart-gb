/// ROM size code at header offset 0x0148 (Pan Docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RomSize {
    /// 32 KiB, 2 banks (no banking)
    Kb32 = 0x00,
    /// 64 KiB, 4 banks
    Kb64 = 0x01,
    /// 128 KiB, 8 banks
    Kb128 = 0x02,
    /// 256 KiB, 16 banks
    Kb256 = 0x03,
    /// 512 KiB, 32 banks
    Kb512 = 0x04,
    /// 1 MiB, 64 banks
    Mb1 = 0x05,
    /// 2 MiB, 128 banks
    Mb2 = 0x06,
    /// 4 MiB, 256 banks
    Mb4 = 0x07,
    /// 8 MiB, 512 banks
    Mb8 = 0x08,
    /// 1.1 MiB, 72 banks (rare)
    Mb1_1 = 0x52,
    /// 1.2 MiB, 80 banks (rare)
    Mb1_2 = 0x53,
    /// 1.5 MiB, 96 banks (rare)
    Mb1_5 = 0x54,
}

impl RomSize {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Kb32),
            0x01 => Some(Self::Kb64),
            0x02 => Some(Self::Kb128),
            0x03 => Some(Self::Kb256),
            0x04 => Some(Self::Kb512),
            0x05 => Some(Self::Mb1),
            0x06 => Some(Self::Mb2),
            0x07 => Some(Self::Mb4),
            0x08 => Some(Self::Mb8),
            0x52 => Some(Self::Mb1_1),
            0x53 => Some(Self::Mb1_2),
            0x54 => Some(Self::Mb1_5),
            _ => None,
        }
    }

    /// Human-readable ROM size (e.g. `"32 KiB"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Kb32 => "32 KiB",
            Self::Kb64 => "64 KiB",
            Self::Kb128 => "128 KiB",
            Self::Kb256 => "256 KiB",
            Self::Kb512 => "512 KiB",
            Self::Mb1 => "1 MiB",
            Self::Mb2 => "2 MiB",
            Self::Mb4 => "4 MiB",
            Self::Mb8 => "8 MiB",
            Self::Mb1_1 => "1.1 MiB",
            Self::Mb1_2 => "1.2 MiB",
            Self::Mb1_5 => "1.5 MiB",
        }
    }

    /// Number of 16 KiB ROM banks.
    pub fn bank_count(self) -> u16 {
        match self {
            Self::Kb32 => 2,
            Self::Kb64 => 4,
            Self::Kb128 => 8,
            Self::Kb256 => 16,
            Self::Kb512 => 32,
            Self::Mb1 => 64,
            Self::Mb2 => 128,
            Self::Mb4 => 256,
            Self::Mb8 => 512,
            Self::Mb1_1 => 72,
            Self::Mb1_2 => 80,
            Self::Mb1_5 => 96,
        }
    }

    /// Total ROM size in bytes.
    pub fn size_bytes(self) -> usize {
        usize::from(self.bank_count()) * 16 * 1024
    }
}

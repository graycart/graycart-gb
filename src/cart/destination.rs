/// Destination code at header offset 0x014A (Pan Docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Destination {
    /// Japan (and possibly overseas)
    Japan = 0x00,
    /// Overseas only
    Overseas = 0x01,
}

impl Destination {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Japan),
            0x01 => Some(Self::Overseas),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Japan => "Japan (and possibly overseas)",
            Self::Overseas => "Overseas only",
        }
    }
}

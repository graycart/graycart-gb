//! Host-agnostic Game Boy buttons.

/// The eight DMG buttons. Host key/controller IDs never appear here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameBoyButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

impl GameBoyButton {
    /// All buttons in a stable order (for tests / frontend iteration).
    pub const ALL: [Self; 8] = [
        Self::Right,
        Self::Left,
        Self::Up,
        Self::Down,
        Self::A,
        Self::B,
        Self::Select,
        Self::Start,
    ];

    /// Bit in the pressed bitmask (`1 << index`).
    pub(crate) fn mask(self) -> u8 {
        1 << self.index()
    }

    fn index(self) -> u8 {
        match self {
            Self::Right => 0,
            Self::Left => 1,
            Self::Up => 2,
            Self::Down => 3,
            Self::A => 4,
            Self::B => 5,
            Self::Select => 6,
            Self::Start => 7,
        }
    }

    /// Which P1 input line (bits 0–3) this button drives.
    pub(crate) fn line_bit(self) -> u8 {
        match self {
            Self::Right | Self::A => 0,
            Self::Left | Self::B => 1,
            Self::Up | Self::Select => 2,
            Self::Down | Self::Start => 3,
        }
    }

    pub(crate) fn is_direction(self) -> bool {
        matches!(self, Self::Right | Self::Left | Self::Up | Self::Down)
    }
}

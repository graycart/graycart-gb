//! CGB CPU vs fixed-dot (PPU/APU) clock domains.
//!
//! Double speed doubles the CPU, not LCD or APU timings (Pan Docs KEY1).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClockState {
    #[default]
    Normal,
    Double,
}

/// CPU T-cycles the CPU is paused after an armed `STOP` speed switch (2050 M-cycles).
pub const STOP_PAUSE_CPU_T: u32 = 8200;

impl ClockState {
    pub fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Double,
            Self::Double => Self::Normal,
        }
    }

    pub fn is_double(self) -> bool {
        matches!(self, Self::Double)
    }

    /// Convert CPU T-cycles to PPU/APU (fixed-dot) T-cycles.
    /// Normal: 1:1. Double: 2 CPU T = 1 fixed T. `leftover` is 0 or 1 (odd CPU T).
    pub fn cpu_t_to_fixed_t(self, cpu_t: u32, leftover: &mut u8) -> u32 {
        match self {
            Self::Normal => cpu_t,
            Self::Double => {
                let total = cpu_t + u32::from(*leftover);
                *leftover = (total & 1) as u8;
                total / 2
            }
        }
    }
}

#[cfg(test)]
mod tests;

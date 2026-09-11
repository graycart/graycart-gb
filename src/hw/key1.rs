//! CGB KEY1/SPD (`$FF4D`): prepare speed switch.
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html): bit 7 is current speed
//! (read-only), bit 0 is switch-armed (R/W). Bits 1–6 read as 1. Writes ignore
//! bit 7. Armed + `STOP` clears armed and toggles speed.

pub const KEY1: u16 = 0xFF4D;

/// Bits 1–6 are unused and read as 1.
const UNUSED_READ_ONES: u8 = 0x7E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key1 {
    armed: bool,
    current_double: bool,
}

impl Default for Key1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Key1 {
    pub fn new() -> Self {
        Self {
            armed: false,
            current_double: false,
        }
    }

    pub fn read(self) -> u8 {
        let mut value = UNUSED_READ_ONES;
        if self.current_double {
            value |= 1 << 7;
        }
        if self.armed {
            value |= 1;
        }
        value
    }

    pub fn write(&mut self, value: u8) {
        self.armed = value & 1 != 0;
    }

    /// If armed, clear armed, toggle current speed, return true (caller runs STOP pause).
    pub fn try_switch_on_stop(&mut self) -> bool {
        if !self.armed {
            return false;
        }
        self.armed = false;
        self.current_double = !self.current_double;
        true
    }

    pub fn current_double(self) -> bool {
        self.current_double
    }

    pub fn armed(self) -> bool {
        self.armed
    }
}

#[cfg(test)]
mod tests;

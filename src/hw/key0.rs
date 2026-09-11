//! CGB KEY0 (`$FF4C`): boot ROM writes this, then it locks after `$FF50`.
//!
//! [Pan Docs](https://gbdev.io/pandocs/CGB_Registers.html). Fast skip-boot starts
//! already unmapped, so KEY0 is locked.

pub const KEY0: u16 = 0xFF4C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key0 {
    value: u8,
    locked: bool,
}

impl Default for Key0 {
    fn default() -> Self {
        Self::new()
    }
}

impl Key0 {
    /// Power-on: `$00`, unlocked (boot ROM has not finished).
    pub fn new() -> Self {
        Self {
            value: 0,
            locked: false,
        }
    }

    /// `$FF50` unmap / Fast: further `write`s are ignored.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn write(&mut self, v: u8) {
        if self.locked {
            return;
        }
        self.value = v;
    }

    pub fn read(self) -> u8 {
        self.value
    }

    /// Fast skip-boot: Native stores cart header `$0143`; Compat stores `$04`.
    pub fn set_and_lock(&mut self, v: u8) {
        self.value = v;
        self.locked = true;
    }

    pub fn is_locked(self) -> bool {
        self.locked
    }
}

#[cfg(test)]
mod tests;

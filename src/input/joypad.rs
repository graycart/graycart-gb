//! `$FF00` / P1 joypad matrix and IF.4 falling-edge request.
//!
//! [Pan Docs — Joypad Input](https://gbdev.io/pandocs/Joypad_Input.html)
//! [Pan Docs — Interrupt Sources](https://gbdev.io/pandocs/Interrupt_Sources.html)

use super::GameBoyButton;

/// `$FF00` P1/JOYP.
pub const P1: u16 = 0xFF00;

/// Select d-pad (bit 4): `0` = selected.
const SELECT_DPAD: u8 = 0x10;
/// Select action buttons (bit 5): `0` = selected.
const SELECT_BUTTONS: u8 = 0x20;
const SELECT_MASK: u8 = SELECT_DPAD | SELECT_BUTTONS;
/// Unused bits 6–7 read as 1.
const UNUSED_HIGH: u8 = 0xC0;

/// Game Boy joypad: select lines + button matrix + edge IRQ signal.
#[derive(Debug, Clone)]
pub struct Joypad {
    /// Writable select bits 4–5 (`0` = select that group).
    select: u8,
    /// Pressed buttons as a bitmask ([`GameBoyButton::mask`]).
    pressed: u8,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Joypad {
    /// Idle: both groups selected, no buttons pressed (`P1` = `$CF`).
    pub fn new() -> Self {
        Self {
            select: 0,
            pressed: 0,
        }
    }

    /// Power-on reset (same defaults as [`Self::new`]).
    pub fn power_on_reset(&mut self) {
        *self = Self::new();
    }

    pub fn press(&mut self, button: GameBoyButton) -> bool {
        let before = self.input_nibble();
        self.pressed |= button.mask();
        falling_edge(before, self.input_nibble())
    }

    pub fn release(&mut self, button: GameBoyButton) -> bool {
        let before = self.input_nibble();
        self.pressed &= !button.mask();
        falling_edge(before, self.input_nibble())
    }

    pub fn is_pressed(&self, button: GameBoyButton) -> bool {
        self.pressed & button.mask() != 0
    }

    /// Read `$FF00`: unused high bits | select | active-low inputs.
    pub fn read(&self) -> u8 {
        UNUSED_HIGH | (self.select & SELECT_MASK) | self.input_nibble()
    }

    /// Write select bits 4–5. Returns `true` if a visible 1→0 edge occurred.
    pub fn write(&mut self, value: u8) -> bool {
        let before = self.input_nibble();
        self.select = value & SELECT_MASK;
        falling_edge(before, self.input_nibble())
    }

    fn input_nibble(&self) -> u8 {
        let dpad = self.select & SELECT_DPAD == 0;
        let buttons = self.select & SELECT_BUTTONS == 0;
        match (dpad, buttons) {
            (false, false) => 0x0F,
            (true, false) => self.group_nibble(true),
            (false, true) => self.group_nibble(false),
            (true, true) => self.group_nibble(true) & self.group_nibble(false),
        }
    }

    /// Active-low nibble for directions (`dpad`) or actions (`!dpad`).
    fn group_nibble(&self, dpad: bool) -> u8 {
        let mut nibble = 0x0F;
        for button in GameBoyButton::ALL {
            if dpad != button.is_direction() {
                continue;
            }
            if self.pressed & button.mask() != 0 {
                nibble &= !(1 << button.line_bit());
            }
        }
        nibble
    }
}

fn falling_edge(before: u8, after: u8) -> bool {
    (before & !after) & 0x0F != 0
}

impl Joypad {
    pub(crate) fn snapshot(&self) -> crate::snapshot::JoypadStateV1 {
        crate::snapshot::JoypadStateV1 {
            select: self.select,
            pressed: self.pressed,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, state: &crate::snapshot::JoypadStateV1) {
        self.select = state.select;
        self.pressed = state.pressed;
    }

    /// Replace pressed buttons without generating IF edges (post load-state sync).
    pub(crate) fn set_pressed_mask(&mut self, mask: u8) {
        self.pressed = mask;
    }
}

#[cfg(test)]
mod tests;

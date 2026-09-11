//! Game Boy timer / divider ([Pan Docs](https://gbdev.io/pandocs/Timer_and_Divider_Registers.html)).
//!
//! Falling-edge TIMA clock from the 16-bit divider, with a 4 T-cycle TIMA reload
//! delay after overflow before TMA is loaded and IF.2 is requested. DIV-write
//! edge-case TIMA bumps are deferred.

/// `FF04` DIV — upper 8 bits of the internal divider.
pub const DIV: u16 = 0xFF04;
/// `FF05` TIMA — timer counter.
pub const TIMA: u16 = 0xFF05;
/// `FF06` TMA — timer modulo (reload value).
pub const TMA: u16 = 0xFF06;
/// `FF07` TAC — timer control.
pub const TAC: u16 = 0xFF07;

const TAC_ENABLE: u8 = 0b0000_0100;

/// Which bit of the 16-bit divider produces TIMA clocks (falling edge).
fn tac_input_bit(tac: u8) -> u16 {
    match tac & 0b11 {
        0b00 => 1 << 9, // 4096 Hz
        0b01 => 1 << 3, // 262144 Hz
        0b10 => 1 << 5, // 65536 Hz
        0b11 => 1 << 7, // 16384 Hz
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    /// 16-bit system divider; [`DIV`] is the high byte.
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    /// T-cycles remaining until TIMA reloads from TMA after overflow (`0` = idle).
    /// When `1`, the next tick reloads (TIMA writes are ignored).
    reload_delay: u8,
    /// CGB speed-switch pause: DIV/TIMA (and reload delay) do not advance.
    div_frozen: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            reload_delay: 0,
            div_frozen: false,
        }
    }

    /// Power-on reset (same defaults as [`Self::new`]).
    pub fn power_on_reset(&mut self) {
        *self = Self::new();
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        match addr {
            DIV => Some((self.div >> 8) as u8),
            TIMA => Some(self.tima),
            TMA => Some(self.tma),
            TAC => Some(self.tac | 0xF8), // unused bits often read as 1
            _ => None,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        match addr {
            DIV => {
                // Writing DIV resets the whole divider. Edge-case TIMA bumps ignored for now.
                self.div = 0;
                true
            }
            TIMA => {
                // Reload cycle: write ignored, TMA load still happens on next tick.
                if self.reload_delay == 1 {
                    return true;
                }
                // During the delay window, a TIMA write cancels reload + IRQ.
                if self.reload_delay > 1 {
                    self.reload_delay = 0;
                }
                self.tima = value;
                true
            }
            TMA => {
                self.tma = value;
                true
            }
            TAC => {
                self.tac = value & 0b111;
                true
            }
            _ => false,
        }
    }

    /// Freeze DIV/TIMA during CGB KEY1 `STOP` pause (Pan Docs: DIV does not tick).
    pub fn freeze_div(&mut self, frozen: bool) {
        self.div_frozen = frozen;
    }

    /// Advance by `t_cycles` CPU clocks. Returns `true` if TIMA reloaded (request IF.2).
    pub fn tick(&mut self, t_cycles: u32) -> bool {
        if self.div_frozen {
            return false;
        }
        let mut irq = false;
        for _ in 0..t_cycles {
            if self.reload_delay > 0 {
                self.reload_delay -= 1;
                if self.reload_delay == 0 {
                    self.tima = self.tma;
                    irq = true;
                }
            }

            let old = self.div;
            self.div = self.div.wrapping_add(1);
            if self.tac & TAC_ENABLE == 0 {
                continue;
            }
            let bit = tac_input_bit(self.tac);
            let falling = old & bit != 0 && self.div & bit == 0;
            if !falling {
                continue;
            }
            let (next, carry) = self.tima.overflowing_add(1);
            if carry {
                self.tima = 0;
                self.reload_delay = 4;
            } else {
                self.tima = next;
            }
        }
        irq
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::TimerStateV1 {
        crate::snapshot::TimerStateV1 {
            div: self.div,
            tima: self.tima,
            tma: self.tma,
            tac: self.tac,
            reload_delay: self.reload_delay,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, state: &crate::snapshot::TimerStateV1) {
        self.div = state.div;
        self.tima = state.tima;
        self.tma = state.tma;
        self.tac = state.tac;
        self.reload_delay = state.reload_delay;
    }
}

#[cfg(test)]
mod tests;

//! Sound Channel 4 — LFSR noise.
//!
//! [Pan Docs — Audio Registers / CH4](https://gbdev.io/pandocs/Audio_Registers.html)

#[derive(Debug, Clone)]
pub(crate) struct NoiseChannel {
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,

    enabled: bool,
    dac_enabled: bool,
    /// 15-bit LFSR state (bits 0..=14).
    lfsr: u16,
    timer: u32,
    volume: u8,
    envelope_timer: u8,
    length_counter: u8,
}

impl NoiseChannel {
    pub(crate) fn new() -> Self {
        Self {
            nr41: 0,
            nr42: 0,
            nr43: 0,
            nr44: 0,
            enabled: false,
            dac_enabled: false,
            lfsr: 0,
            timer: 0,
            volume: 0,
            envelope_timer: 0,
            length_counter: 0,
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    pub(crate) fn volume(&self) -> u8 {
        self.volume
    }

    #[cfg(test)]
    pub(crate) fn lfsr(&self) -> u16 {
        self.lfsr & 0x7FFF
    }

    pub(crate) fn power_off(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn read_nr42(&self) -> u8 {
        self.nr42
    }
    pub(crate) fn read_nr43(&self) -> u8 {
        self.nr43
    }
    pub(crate) fn read_nr44(&self) -> u8 {
        self.nr44 | 0xBF
    }

    pub(crate) fn write_nr41(&mut self, value: u8) {
        self.nr41 = value & 0x3F;
        self.length_counter = 64 - (value & 0x3F);
    }

    pub(crate) fn write_nr42(&mut self, value: u8) {
        self.nr42 = value;
        self.dac_enabled = value & 0xF8 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub(crate) fn write_nr43(&mut self, value: u8) {
        self.nr43 = value;
    }

    pub(crate) fn write_nr44(&mut self, value: u8, _next_frame_length: bool) {
        self.nr44 = value & 0xC0;
        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    /// T-cycles between LFSR clocks, or `None` when shift is 14/15 (frozen).
    pub(crate) fn period_t(&self) -> Option<u32> {
        let shift = (self.nr43 >> 4) & 0x0F;
        if shift >= 14 {
            return None;
        }
        let divider = self.nr43 & 0x07;
        // divider 0 is treated as 0.5 → half the usual 16× base.
        let base = if divider == 0 {
            8
        } else {
            16 * u32::from(divider)
        };
        Some(base << shift)
    }

    fn width7(&self) -> bool {
        self.nr43 & 0x08 != 0
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.timer = self.period_t().unwrap_or(0);
        self.volume = self.nr42 >> 4;
        self.envelope_timer = self.nr42 & 0x07;
        if self.envelope_timer == 0 {
            self.envelope_timer = 8;
        }
        // Pan Docs: LFSR cleared on (re)trigger; XNOR step escapes the all-zero state.
        self.lfsr = 0;
    }

    /// One LFSR step (Pan Docs XNOR into bit 15, optional 7-bit tap, then shift).
    pub(crate) fn clock_lfsr(&mut self) {
        let b0 = self.lfsr & 1;
        let b1 = (self.lfsr >> 1) & 1;
        let bit = u16::from(b0 == b1); // XNOR → 1 if identical
        // Write into bit 15 of a 16-bit view, mirror to bit 7 in short mode, then >> 1.
        let mut next = self.lfsr | (bit << 15);
        if self.width7() {
            next = (next & !(1 << 7)) | (bit << 7);
        }
        self.lfsr = (next >> 1) & 0x7FFF;
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) {
        if !self.enabled {
            return;
        }
        let Some(period) = self.period_t() else {
            return; // shift 14/15: no LFSR clocks
        };
        let mut left = t_cycles;
        while left > 0 {
            if self.timer == 0 {
                self.timer = period.max(1);
            }
            let step = left.min(self.timer);
            self.timer -= step;
            left -= step;
            if self.timer == 0 {
                self.clock_lfsr();
                self.timer = period.max(1);
            }
        }
    }

    pub(crate) fn clock_length(&mut self) {
        if self.nr44 & 0x40 == 0 {
            return;
        }
        if self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    pub(crate) fn clock_envelope(&mut self) {
        if !self.enabled {
            return;
        }
        let period = self.nr42 & 0x07;
        if period == 0 {
            return;
        }
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }
        if self.envelope_timer == 0 {
            self.envelope_timer = period;
            let increase = self.nr42 & 0x08 != 0;
            if increase && self.volume < 15 {
                self.volume += 1;
            } else if !increase && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    /// Generator digital output `$0`–`$F` (disabled → 0; LFSR bit0 selects volume).
    pub(crate) fn digital_output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        if self.lfsr & 1 == 0 {
            0
        } else {
            self.volume & 0x0F
        }
    }

    /// DAC output ≈ `-1.0..=1.0` (0 when DAC/channel off).
    ///
    /// LFSR bit0 = 0 → low (−amp); bit0 = 1 → high (+amp).
    pub(crate) fn dac_output(&self) -> f32 {
        if !self.dac_enabled || !self.enabled {
            return 0.0;
        }
        let amp = f32::from(self.volume) / 15.0;
        if self.lfsr & 1 == 0 { -amp } else { amp }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::NoiseStateV1 {
        crate::snapshot::NoiseStateV1 {
            enabled: self.enabled,
            dac_on: self.dac_enabled,
            length: self.length_counter,
            length_enable: self.nr44 & 0x40 != 0,
            envelope_volume: self.volume,
            envelope_dir: self.nr42 & 0x08 != 0,
            envelope_period: self.nr42 & 0x07,
            envelope_timer: self.envelope_timer,
            clock_shift: (self.nr43 >> 4) & 0x0F,
            width_mode: self.nr43 & 0x08 != 0,
            divisor_code: self.nr43 & 0x07,
            lfsr: self.lfsr & 0x7FFF,
            timer: self.timer.min(u16::MAX as u32) as u16,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, s: &crate::snapshot::NoiseStateV1) {
        self.enabled = s.enabled;
        self.dac_enabled = s.dac_on;
        self.length_counter = s.length;
        self.volume = s.envelope_volume;
        self.envelope_timer = s.envelope_timer;
        self.lfsr = s.lfsr;
        self.timer = u32::from(s.timer);
        self.nr41 = 64u8.wrapping_sub(s.length) & 0x3F;
        self.nr42 = (s.envelope_volume << 4)
            | (if s.envelope_dir { 0x08 } else { 0 })
            | (s.envelope_period & 0x07)
            | if s.dac_on { 0xF0 } else { 0 };
        self.nr43 =
            (s.clock_shift << 4) | (if s.width_mode { 0x08 } else { 0 }) | (s.divisor_code & 0x07);
        self.nr44 = if s.length_enable { 0x40 } else { 0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_clears_lfsr_and_xnor_escapes_zero() {
        let mut ch = NoiseChannel::new();
        ch.write_nr42(0xF0);
        ch.write_nr43(0x00); // 15-bit, fastest-ish
        ch.write_nr44(0x80, false);
        assert_eq!(ch.lfsr(), 0);
        ch.clock_lfsr();
        // XNOR(0,0)=1 → after shift, bit 14 set.
        assert_ne!(ch.lfsr(), 0);
        assert_eq!(ch.lfsr() & (1 << 14), 1 << 14);
    }

    #[test]
    fn width7_mirrors_into_bit6_after_shift() {
        let mut ch = NoiseChannel::new();
        ch.write_nr42(0xF0);
        ch.write_nr43(0x08); // 7-bit width
        ch.write_nr44(0x80, false);
        ch.clock_lfsr();
        // bit written also lands at bit 7 pre-shift → bit 6 post-shift.
        assert_eq!(ch.lfsr() & (1 << 6), 1 << 6);
    }

    #[test]
    fn period_divisor_zero_is_half() {
        let mut ch = NoiseChannel::new();
        ch.write_nr43(0x00); // div 0, shift 0
        assert_eq!(ch.period_t(), Some(8));
        ch.write_nr43(0x01); // div 1, shift 0
        assert_eq!(ch.period_t(), Some(16));
        ch.write_nr43(0x11); // div 1, shift 1
        assert_eq!(ch.period_t(), Some(32));
        ch.write_nr43(0xE0); // shift 14 → frozen
        assert_eq!(ch.period_t(), None);
    }

    #[test]
    fn dac_off_disables() {
        let mut ch = NoiseChannel::new();
        ch.write_nr42(0xF0);
        ch.write_nr44(0x80, false);
        assert!(ch.enabled());
        ch.write_nr42(0x07);
        assert!(!ch.enabled());
        assert_eq!(ch.dac_output(), 0.0);
    }

    #[test]
    fn envelope_decreases_volume() {
        let mut ch = NoiseChannel::new();
        ch.write_nr42(0xF1); // vol 15, decrease, period 1
        ch.write_nr44(0x80, false);
        assert_eq!(ch.volume(), 15);
        ch.clock_envelope();
        assert_eq!(ch.volume(), 14);
    }
}

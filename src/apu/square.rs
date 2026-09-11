//! Shared pulse/square channel core (CH1 and CH2).
//!
//! Sweep lives only on Channel 1 — see [`super::Channel1`].

const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

/// Duty / length / envelope / frequency / trigger / DAC — no sweep.
#[derive(Debug, Clone)]
pub(crate) struct SquareChannel {
    nrx1: u8,
    nrx2: u8,
    nrx3: u8,
    nrx4: u8,

    enabled: bool,
    dac_enabled: bool,
    /// Frequency timer period counter (T-cycles).
    timer: u32,
    duty_step: u8,
    volume: u8,
    envelope_timer: u8,
    length_counter: u8,
}

impl SquareChannel {
    pub(crate) fn new() -> Self {
        Self {
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,
            enabled: false,
            dac_enabled: false,
            timer: 0,
            duty_step: 0,
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

    pub(crate) fn duty(&self) -> u8 {
        self.nrx1 >> 6
    }

    pub(crate) fn frequency(&self) -> u16 {
        u16::from(self.nrx3) | (u16::from(self.nrx4 & 0x07) << 8)
    }

    /// Write 11-bit period into NRx3/NRx4 (used by CH1 sweep).
    pub(crate) fn set_frequency(&mut self, freq: u16) {
        let freq = freq & 0x07FF;
        self.nrx3 = (freq & 0xFF) as u8;
        self.nrx4 = (self.nrx4 & !0x07) | ((freq >> 8) as u8 & 0x07);
    }

    pub(crate) fn disable(&mut self) {
        self.enabled = false;
    }

    pub(crate) fn power_off(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn read_nrx1(&self) -> u8 {
        self.nrx1 | 0x3F
    }
    pub(crate) fn read_nrx2(&self) -> u8 {
        self.nrx2
    }
    pub(crate) fn read_nrx4(&self) -> u8 {
        self.nrx4 | 0xBF
    }

    pub(crate) fn write_nrx1(&mut self, value: u8) {
        self.nrx1 = value;
        self.length_counter = 64 - (value & 0x3F);
    }
    pub(crate) fn write_nrx2(&mut self, value: u8) {
        self.nrx2 = value;
        self.dac_enabled = value & 0xF8 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }
    pub(crate) fn write_nrx3(&mut self, value: u8) {
        self.nrx3 = value;
    }
    pub(crate) fn write_nrx4(&mut self, value: u8, _next_frame_length: bool) {
        let length_enable = value & 0x40 != 0;
        let was_length = self.nrx4 & 0x40 != 0;
        self.nrx4 = value & 0xC7;

        if length_enable && !was_length && self.length_counter == 0 {
            // Obscure: enabling length with zero counter may reload — skip for slice.
        }

        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    fn period_t(&self) -> u32 {
        (2048u32.saturating_sub(u32::from(self.frequency()))) * 4
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.timer = self.period_t();
        self.volume = self.nrx2 >> 4;
        self.envelope_timer = self.nrx2 & 0x07;
        if self.envelope_timer == 0 {
            self.envelope_timer = 8; // period 0 treated as 8 on DMG for envelope
        }
        self.duty_step = 0;
    }

    /// Shadow frequency after trigger (for CH1 sweep).
    pub(crate) fn frequency_after_trigger(&self) -> u16 {
        self.frequency()
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) {
        if !self.enabled {
            return;
        }
        let mut left = t_cycles;
        while left > 0 {
            if self.timer == 0 {
                self.timer = self.period_t().max(1);
            }
            let step = left.min(self.timer);
            self.timer -= step;
            left -= step;
            if self.timer == 0 {
                self.duty_step = (self.duty_step + 1) & 7;
                self.timer = self.period_t().max(1);
            }
        }
    }

    pub(crate) fn clock_length(&mut self) {
        if self.nrx4 & 0x40 == 0 {
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
        let period = self.nrx2 & 0x07;
        if period == 0 {
            return;
        }
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }
        if self.envelope_timer == 0 {
            self.envelope_timer = period;
            let increase = self.nrx2 & 0x08 != 0;
            if increase && self.volume < 15 {
                self.volume += 1;
            } else if !increase && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    /// Generator digital output `$0`–`$F` (disabled/silent → 0).
    pub(crate) fn digital_output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        let duty = self.duty() as usize;
        let bit = DUTY[duty][self.duty_step as usize];
        if bit == 0 { 0 } else { self.volume & 0x0F }
    }

    /// DAC output in roughly `-1.0..=1.0` (0 when DAC/channel off).
    pub(crate) fn dac_output(&self) -> f32 {
        if !self.dac_enabled || !self.enabled {
            return 0.0;
        }
        let duty = self.duty() as usize;
        let bit = DUTY[duty][self.duty_step as usize];
        let amp = f32::from(self.volume) / 15.0;
        if bit == 0 { -amp } else { amp }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::SquareStateV1 {
        crate::snapshot::SquareStateV1 {
            enabled: self.enabled,
            dac_on: self.dac_enabled,
            length: self.length_counter,
            length_enable: self.nrx4 & 0x40 != 0,
            duty: self.duty(),
            envelope_volume: self.volume,
            envelope_dir: self.nrx2 & 0x08 != 0,
            envelope_period: self.nrx2 & 0x07,
            envelope_timer: self.envelope_timer,
            frequency: self.frequency(),
            frequency_timer: self.timer.min(u16::MAX as u32) as u16,
            duty_pos: self.duty_step,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, s: &crate::snapshot::SquareStateV1) {
        self.enabled = s.enabled;
        self.dac_enabled = s.dac_on;
        self.length_counter = s.length;
        self.volume = s.envelope_volume;
        self.envelope_timer = s.envelope_timer;
        self.timer = u32::from(s.frequency_timer);
        self.duty_step = s.duty_pos;
        self.nrx1 = (s.duty << 6) | (64u8.wrapping_sub(s.length) & 0x3F);
        self.nrx2 = (s.envelope_volume << 4)
            | (if s.envelope_dir { 0x08 } else { 0 })
            | (s.envelope_period & 0x07)
            | if s.dac_on { 0xF0 } else { 0 };
        self.nrx3 = (s.frequency & 0xFF) as u8;
        self.nrx4 = ((s.frequency >> 8) as u8 & 0x07) | if s.length_enable { 0x40 } else { 0 };
    }
}

/// Channel 1: square pulse + NR10 period sweep.
#[derive(Debug, Clone)]
pub(crate) struct Channel1 {
    pub pulse: SquareChannel,
    nr10: u8,
    shadow_freq: u16,
    sweep_timer: u8,
    sweep_enabled: bool,
    /// Set when a subtract-mode calculation has run since last trigger
    /// (clearing NR10 negate bit then disables CH1 — Pan Docs obscure).
    negate_used: bool,
}

impl Channel1 {
    pub(crate) fn new() -> Self {
        Self {
            pulse: SquareChannel::new(),
            nr10: 0,
            shadow_freq: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            negate_used: false,
        }
    }

    pub(crate) fn power_off(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn read_nr10(&self) -> u8 {
        self.nr10 | 0x80
    }

    pub(crate) fn write_nr10(&mut self, value: u8) {
        let old_negate = self.nr10 & 0x08 != 0;
        self.nr10 = value & 0x7F;
        let new_negate = self.nr10 & 0x08 != 0;
        // Clearing negate after a subtract calc since trigger → disable.
        if old_negate && !new_negate && self.negate_used {
            self.pulse.disable();
        }
    }

    pub(crate) fn write_nr14(&mut self, value: u8, next_frame_length: bool) {
        self.pulse.write_nrx4(value, next_frame_length);
        if value & 0x80 != 0 {
            self.trigger_sweep();
        }
    }

    fn pace(&self) -> u8 {
        (self.nr10 >> 4) & 0x07
    }

    fn shift(&self) -> u8 {
        self.nr10 & 0x07
    }

    fn negate(&self) -> bool {
        self.nr10 & 0x08 != 0
    }

    fn reload_sweep_timer(&mut self) {
        let pace = self.pace();
        // Period 0 treated as 8 (Pan Docs obscure / Audio details).
        self.sweep_timer = if pace == 0 { 8 } else { pace };
    }

    fn trigger_sweep(&mut self) {
        self.shadow_freq = self.pulse.frequency_after_trigger();
        self.reload_sweep_timer();
        self.sweep_enabled = self.pace() != 0 || self.shift() != 0;
        self.negate_used = false;
        // Immediate overflow check when shift != 0 (result not written back).
        if self.shift() != 0 {
            let _ = self.calculate_sweep(false);
        }
    }

    /// Compute next shadow frequency. On overflow, disables CH1 and returns `None`.
    /// When `write_back`, updates shadow + pulse frequency.
    fn calculate_sweep(&mut self, write_back: bool) -> Option<u16> {
        let offset = self.shadow_freq >> self.shift();
        let new_freq = if self.negate() {
            self.negate_used = true;
            self.shadow_freq.wrapping_sub(offset)
        } else {
            self.shadow_freq.wrapping_add(offset)
        };
        if new_freq > 0x07FF {
            self.pulse.disable();
            return None;
        }
        if write_back && self.shift() != 0 {
            self.shadow_freq = new_freq;
            self.pulse.set_frequency(new_freq);
        }
        Some(new_freq)
    }

    /// Clocked at 128 Hz by the frame sequencer.
    pub(crate) fn clock_sweep(&mut self) {
        if !self.sweep_enabled {
            return;
        }
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer != 0 {
            return;
        }
        self.reload_sweep_timer();

        // Pace 0: timer still reloads, but iterations are disabled.
        if self.pace() == 0 {
            return;
        }

        if let Some(_freq) = self.calculate_sweep(true) {
            // Second overflow check with the new shadow; do not write back.
            let _ = self.calculate_sweep(false);
        }
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::Ch1StateV1 {
        crate::snapshot::Ch1StateV1 {
            square: self.pulse.snapshot(),
            sweep_period: (self.nr10 >> 4) & 0x07,
            sweep_negate: self.nr10 & 0x08 != 0,
            sweep_shift: self.nr10 & 0x07,
            sweep_timer: self.sweep_timer,
            sweep_enable: self.sweep_enabled,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, s: &crate::snapshot::Ch1StateV1) {
        self.pulse.apply_snapshot(&s.square);
        self.nr10 = (s.sweep_period << 4)
            | (if s.sweep_negate { 0x08 } else { 0 })
            | (s.sweep_shift & 0x07);
        self.sweep_timer = s.sweep_timer;
        self.sweep_enabled = s.sweep_enable;
        self.shadow_freq = self.pulse.frequency();
        self.negate_used = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_enables_when_dac_on() {
        let mut ch = SquareChannel::new();
        ch.write_nrx2(0xF0); // max volume, dac on
        ch.write_nrx1(0x80); // duty 2, length
        ch.write_nrx3(0x00);
        ch.write_nrx4(0x87, false); // trigger + high freq bits
        assert!(ch.enabled());
        assert!(ch.dac_output() >= 0.0);
    }

    #[test]
    fn dac_off_disables_channel() {
        let mut ch = SquareChannel::new();
        ch.write_nrx2(0xF0);
        ch.write_nrx4(0x80, false);
        assert!(ch.enabled());
        ch.write_nrx2(0x07); // dac off
        assert!(!ch.enabled());
        assert_eq!(ch.dac_output(), 0.0);
    }

    #[test]
    fn sweep_increases_frequency_over_time() {
        let mut ch1 = Channel1::new();
        ch1.pulse.write_nrx2(0xF0);
        ch1.pulse.write_nrx1(0x80);
        ch1.write_nr10(0x13); // pace 1, shift 3, add
        ch1.pulse.write_nrx3(0x00);
        // NR14 write supplies period high bits — 0x100 → high=1.
        ch1.write_nr14(0x80 | 0x01, false);
        assert!(ch1.pulse.enabled());
        let start = ch1.pulse.frequency();
        assert_eq!(start, 0x100);
        ch1.clock_sweep();
        let after = ch1.pulse.frequency();
        assert!(
            after > start,
            "add sweep should raise period value: {start} → {after}"
        );
        assert_eq!(after, start + (start >> 3));
    }

    #[test]
    fn sweep_negate_clear_after_subtract_disables() {
        let mut ch1 = Channel1::new();
        ch1.pulse.write_nrx2(0xF0);
        ch1.write_nr10(0x0B); // pace 0, negate, shift 3
        ch1.pulse.write_nrx3(0x00);
        ch1.write_nr14(0x80 | 0x02, false); // freq 0x200
        assert!(
            ch1.pulse.enabled(),
            "subtract calc 0x200-0x40 should not overflow"
        );
        ch1.write_nr10(0x03); // negate cleared
        assert!(!ch1.pulse.enabled(), "negate-used quirk must disable CH1");
    }

    #[test]
    fn sweep_second_overflow_check_disables() {
        // start=0x500, shift=1 → 0x780 OK; second 0x780+0x3C0 overflows.
        let mut ch1 = Channel1::new();
        ch1.pulse.write_nrx2(0xF0);
        ch1.write_nr10(0x11); // pace 1, shift 1, add
        ch1.pulse.write_nrx3(0x00);
        ch1.write_nr14(0x80 | 0x05, false); // freq 0x500
        assert!(ch1.pulse.enabled());
        assert_eq!(ch1.pulse.frequency(), 0x500);
        ch1.clock_sweep();
        assert!(
            !ch1.pulse.enabled(),
            "second overflow check after write-back must disable"
        );
    }

    #[test]
    fn sweep_overflow_on_trigger_disables() {
        let mut ch1 = Channel1::new();
        ch1.pulse.write_nrx2(0xF0);
        ch1.pulse.write_nrx1(0x80);
        ch1.write_nr10(0x01); // pace 0, shift 1, add
        ch1.pulse.write_nrx3(0x00);
        // freq=$700, shift=1, add → 0xA80 > 0x7FF
        ch1.write_nr14(0x80 | 0x07, false);
        assert!(
            !ch1.pulse.enabled(),
            "immediate sweep overflow must disable CH1"
        );
    }
}

//! Sound Channel 3 — wave RAM playback (32 × 4-bit samples).
//!
//! [Pan Docs — Audio Registers / CH3](https://gbdev.io/pandocs/Audio_Registers.html)

use super::registers::WAVE_RAM_START;

#[derive(Debug, Clone)]
pub(crate) struct WaveChannel {
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,

    enabled: bool,
    dac_enabled: bool,
    /// Frequency timer (T-cycles until next nibble).
    timer: u32,
    /// 0..31 sample index into wave RAM nibbles.
    position: u8,
    /// Last digital sample read (0..=15); emitted continuously.
    sample_buffer: u8,
    length_counter: u16,
    /// 16 bytes; each holds high-nibble then low-nibble samples.
    wave_ram: [u8; 16],
}

impl WaveChannel {
    pub(crate) fn new() -> Self {
        Self {
            nr30: 0,
            nr31: 0,
            nr32: 0,
            nr33: 0,
            nr34: 0,
            enabled: false,
            dac_enabled: false,
            timer: 0,
            position: 0,
            sample_buffer: 0,
            length_counter: 0,
            wave_ram: [0; 16],
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    pub(crate) fn frequency(&self) -> u16 {
        u16::from(self.nr33) | (u16::from(self.nr34 & 0x07) << 8)
    }

    pub(crate) fn volume_level(&self) -> u8 {
        // NR32 bits 6-5: 0=mute, 1=100%, 2=50%, 3=25% → report as 0/15/8/4-ish
        match (self.nr32 >> 5) & 0x03 {
            0 => 0,
            1 => 15,
            2 => 8,
            _ => 4,
        }
    }

    /// Clear channel regs/state; wave RAM is preserved (Pan Docs).
    pub(crate) fn power_off(&mut self) {
        let wave_ram = self.wave_ram;
        *self = Self::new();
        self.wave_ram = wave_ram;
        self.sample_buffer = 0;
    }

    pub(crate) fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7F
    }
    pub(crate) fn read_nr32(&self) -> u8 {
        self.nr32 | 0x9F
    }
    pub(crate) fn read_nr34(&self) -> u8 {
        self.nr34 | 0xBF
    }

    pub(crate) fn write_nr30(&mut self, value: u8) {
        self.nr30 = value & 0x80;
        self.dac_enabled = value & 0x80 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub(crate) fn write_nr31(&mut self, value: u8) {
        self.nr31 = value;
        self.length_counter = 256 - u16::from(value);
    }

    pub(crate) fn write_nr32(&mut self, value: u8) {
        self.nr32 = value & 0x60;
    }

    pub(crate) fn write_nr33(&mut self, value: u8) {
        self.nr33 = value;
    }

    pub(crate) fn write_nr34(&mut self, value: u8, _next_frame_length: bool) {
        self.nr34 = value & 0xC7;
        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    pub(crate) fn read_wave(&self, addr: u16) -> u8 {
        let i = (addr - WAVE_RAM_START) as usize;
        self.wave_ram[i]
    }

    pub(crate) fn write_wave(&mut self, addr: u16, value: u8) {
        // First-cut: allow CPU access always. Accurate contention is 8H.
        let i = (addr - WAVE_RAM_START) as usize;
        self.wave_ram[i] = value;
    }

    /// T-cycles between wave RAM nibble advances: `(2048 - freq) * 2`.
    fn period_t(&self) -> u32 {
        (2048u32.saturating_sub(u32::from(self.frequency()))) * 2
    }

    fn output_level(&self) -> u8 {
        (self.nr32 >> 5) & 0x03
    }

    fn nibble_at(&self, position: u8) -> u8 {
        let byte = self.wave_ram[(position / 2) as usize];
        if position & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        }
    }

    fn apply_level(&self, sample: u8) -> u8 {
        match self.output_level() {
            0 => 0,           // mute
            1 => sample,      // 100%
            2 => sample >> 1, // 50%
            3 => sample >> 2, // 25%
            _ => 0,
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
        }
        self.timer = self.period_t().max(2);
        // Position reset; first *new* read advances from here. Sample buffer
        // keeps the previous value until the first timer expiry (Pan Docs).
        self.position = 0;
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) {
        if !self.enabled {
            return;
        }
        let mut left = t_cycles;
        while left > 0 {
            if self.timer == 0 {
                self.timer = self.period_t().max(2);
            }
            let step = left.min(self.timer);
            self.timer -= step;
            left -= step;
            if self.timer == 0 {
                // Advance then fetch — first fetch after trigger is nibble 0
                // (high nibble of $FF30). Obscure “skip first” is deferred to 8H.
                self.sample_buffer = self.nibble_at(self.position);
                self.position = (self.position + 1) & 31;
                self.timer = self.period_t().max(2);
            }
        }
    }

    pub(crate) fn clock_length(&mut self) {
        if self.nr34 & 0x40 == 0 {
            return;
        }
        if self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    /// Generator digital output `$0`–`$F` after NR32 level (disabled → 0).
    pub(crate) fn digital_output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        self.apply_level(self.sample_buffer)
    }

    /// DAC output ≈ `-1.0..=1.0` (0 when DAC/channel off or muted).
    ///
    /// Digital 0..=15 maps with negative slope (0 → +1, 15 → −1) when audible.
    pub(crate) fn dac_output(&self) -> f32 {
        if !self.dac_enabled || !self.enabled {
            return 0.0;
        }
        if self.output_level() == 0 {
            return 0.0;
        }
        let digital = self.apply_level(self.sample_buffer);
        // (7.5 - d) / 7.5  →  1 at d=0,  -1 at d=15
        1.0 - (f32::from(digital) * 2.0 / 15.0)
    }

    pub(crate) fn wave_ram_bytes(&self) -> [u8; 16] {
        self.wave_ram
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::WaveStateV1 {
        crate::snapshot::WaveStateV1 {
            dac_on: self.dac_enabled,
            length: self.length_counter.min(255) as u8,
            length_enable: self.nr34 & 0x40 != 0,
            volume_code: (self.nr32 >> 5) & 0x03,
            frequency: self.frequency(),
            frequency_timer: self.timer.min(u16::MAX as u32) as u16,
            pos_nib: self.position,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, s: &crate::snapshot::WaveStateV1, wave_ram: &[u8; 16]) {
        self.dac_enabled = s.dac_on;
        self.length_counter = u16::from(s.length);
        // V1 has no `enabled` field; infer from DAC + length (matches trigger/clock_length).
        self.enabled = s.dac_on && (self.length_counter > 0 || !s.length_enable);
        self.position = s.pos_nib;
        self.timer = u32::from(s.frequency_timer);
        self.wave_ram = *wave_ram;
        self.nr30 = if s.dac_on { 0x80 } else { 0 };
        self.nr31 = (256u16.wrapping_sub(self.length_counter)) as u8;
        self.nr32 = (s.volume_code << 5) & 0x60;
        self.nr33 = (s.frequency & 0xFF) as u8;
        self.nr34 = ((s.frequency >> 8) as u8 & 0x07) | if s.length_enable { 0x40 } else { 0 };
        self.sample_buffer = self.nibble_at(self.position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_ram_high_nibble_first() {
        let mut ch = WaveChannel::new();
        ch.write_wave(WAVE_RAM_START, 0xAB); // high=A, low=B
        assert_eq!(ch.nibble_at(0), 0x0A);
        assert_eq!(ch.nibble_at(1), 0x0B);
        ch.write_wave(WAVE_RAM_START + 1, 0xC5);
        assert_eq!(ch.nibble_at(2), 0x0C);
        assert_eq!(ch.nibble_at(3), 0x05);
    }

    #[test]
    fn output_level_shifts() {
        let mut ch = WaveChannel::new();
        ch.sample_buffer = 0x0C; // 12
        ch.write_nr32(0x00); // mute
        assert_eq!(ch.apply_level(0x0C), 0);
        ch.write_nr32(0x20); // 100%
        assert_eq!(ch.apply_level(0x0C), 0x0C);
        ch.write_nr32(0x40); // 50%
        assert_eq!(ch.apply_level(0x0C), 0x06);
        ch.write_nr32(0x60); // 25%
        assert_eq!(ch.apply_level(0x0C), 0x03);
    }

    #[test]
    fn dac_off_disables_channel() {
        let mut ch = WaveChannel::new();
        ch.write_nr30(0x80);
        ch.write_nr32(0x20);
        ch.write_nr34(0x80, false);
        assert!(ch.enabled());
        ch.write_nr30(0x00);
        assert!(!ch.enabled());
        assert_eq!(ch.dac_output(), 0.0);
    }

    #[test]
    fn power_off_preserves_wave_ram() {
        let mut ch = WaveChannel::new();
        ch.write_wave(WAVE_RAM_START, 0x42);
        ch.write_nr30(0x80);
        ch.power_off();
        assert_eq!(ch.read_wave(WAVE_RAM_START), 0x42);
        assert!(!ch.dac_enabled());
    }

    #[test]
    fn apply_snapshot_length_expired_keeps_disabled() {
        let mut ch = WaveChannel::new();
        ch.write_nr30(0x80);
        ch.write_nr31(0xFF); // length counter = 1
        ch.write_nr34(0xC0 | 0x80, false); // length enable + trigger
        ch.clock_length();
        assert!(ch.dac_enabled());
        assert!(!ch.enabled());

        let snap = ch.snapshot();
        let mut restored = WaveChannel::new();
        restored.apply_snapshot(&snap, &ch.wave_ram_bytes());
        assert!(restored.dac_enabled());
        assert!(!restored.enabled());
        assert_eq!(restored.dac_output(), 0.0);
    }

    #[test]
    fn apply_snapshot_running_channel_stays_enabled() {
        let mut ch = WaveChannel::new();
        ch.write_nr30(0x80);
        ch.write_nr31(0x00); // length counter = 256 → stored as 255
        ch.write_nr34(0x80, false); // trigger, length clock off
        assert!(ch.enabled());

        let snap = ch.snapshot();
        let mut restored = WaveChannel::new();
        restored.apply_snapshot(&snap, &ch.wave_ram_bytes());
        assert!(restored.enabled());
        assert!(restored.dac_enabled());
    }
}

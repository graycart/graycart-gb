//! DMG APU — Phase 8 (host I/O in `frontend/audio`).
//!
//! Authoritative hardware reference:
//! [Pan Docs — Audio](https://gbdev.io/pandocs/Audio.html)
//!
//! Do not invent behavior from commercial ROM observations when Pan Docs or
//! conformance tests (Blargg `dmg_sound`) disagree.

mod frame_sequencer;
mod mixer;
mod noise;
mod pcm;
mod registers;
mod sample_buffer;
mod square;
mod wave;

pub use pcm::{PCM12, PCM34};
pub use registers::{
    NR10, NR11, NR12, NR13, NR14, NR21, NR22, NR23, NR24, NR30, NR31, NR32, NR33, NR34, NR41, NR42,
    NR43, NR44, NR50, NR51, NR52, WAVE_RAM_END, WAVE_RAM_START,
};
pub use sample_buffer::{HOST_SAMPLE_RATE, SampleBuffer, StereoSample};

use frame_sequencer::FrameSequencer;
use mixer::Mixer;
use noise::NoiseChannel;
use square::{Channel1, SquareChannel};
use wave::WaveChannel;

/// CPU clocks per second (DMG).
pub const CPU_HZ: u32 = 4_194_304;

pub struct Apu {
    powered: bool,
    sequencer: FrameSequencer,
    ch1: Channel1,
    ch2: SquareChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    mixer: Mixer,
    samples: SampleBuffer,
    /// T-cycles per host sample at [`HOST_SAMPLE_RATE`] (≈ 87.38 @ 48 kHz).
    cycles_per_sample: f64,
    sample_phase: f64,
    /// Diagnostics (host-facing counters).
    pub stats: ApuStats,
}

/// Counters / extrema for `--verbose` audio debugging.
#[derive(Debug, Clone)]
pub struct ApuStats {
    pub samples_generated: u64,
    pub nonzero_samples: u64,
    pub min: f32,
    pub max: f32,
}

impl Default for ApuStats {
    fn default() -> Self {
        Self {
            samples_generated: 0,
            nonzero_samples: 0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
        }
    }
}

impl ApuStats {
    fn note_sample(&mut self, s: StereoSample) {
        self.samples_generated = self.samples_generated.saturating_add(1);
        let peak = s.left.abs().max(s.right.abs());
        if peak > 0.0 {
            self.nonzero_samples = self.nonzero_samples.saturating_add(1);
        }
        self.min = self.min.min(s.left).min(s.right);
        self.max = self.max.max(s.left).max(s.right);
    }

    pub fn reset_window(&mut self) {
        *self = Self::default();
    }
}

/// Rate-limited Channel 1 dump for `--verbose`.
#[derive(Debug, Clone, Copy)]
pub struct Ch1Debug {
    pub nr52: u8,
    pub powered: bool,
    pub active: bool,
    pub dac: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub route_l: bool,
    pub route_r: bool,
    pub frequency: u16,
    pub duty: u8,
    pub volume: u8,
    pub samples_generated: u64,
    pub nonzero_samples: u64,
    pub min: f32,
    pub max: f32,
}

impl std::fmt::Display for Ch1Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (min, max) = if self.samples_generated == 0 {
            (0.0, 0.0)
        } else {
            (self.min, self.max)
        };
        writeln!(f, "apu ch1:")?;
        writeln!(
            f,
            "  NR52=${:02X} powered={} active={}",
            self.nr52, self.powered, self.active
        )?;
        writeln!(
            f,
            "  NR51=${:02X} routed L={} R={}",
            self.nr51, self.route_l, self.route_r
        )?;
        writeln!(f, "  NR50=${:02X}", self.nr50)?;
        writeln!(f, "  DAC={}", self.dac)?;
        writeln!(f, "  frequency={}", self.frequency)?;
        writeln!(f, "  duty={}", self.duty)?;
        writeln!(f, "  envelope volume={}", self.volume)?;
        writeln!(
            f,
            "  samples_generated={} nonzero={}",
            self.samples_generated, self.nonzero_samples
        )?;
        write!(f, "  min={min:.3} max={max:.3}")
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        Self {
            powered: false,
            sequencer: FrameSequencer::new(),
            ch1: Channel1::new(),
            ch2: SquareChannel::new(),
            ch3: WaveChannel::new(),
            ch4: NoiseChannel::new(),
            mixer: Mixer::new(),
            samples: SampleBuffer::new(),
            cycles_per_sample: f64::from(CPU_HZ) / f64::from(HOST_SAMPLE_RATE),
            sample_phase: 0.0,
            stats: ApuStats::default(),
        }
    }

    /// Power-on reset (same defaults as [`Self::new`]).
    pub fn power_on_reset(&mut self) {
        *self = Self::new();
    }

    /// DMG register snapshot after the boot ROM hands off (`$0100`).
    ///
    /// Values from Pan Docs power-up sequence (DMG column).
    pub fn apply_after_boot(&mut self) {
        self.power_off();
        self.powered = true;
        self.sequencer.reset();
        // Direct register load (APU already powered).
        self.ch1.write_nr10(0x80);
        self.ch1.pulse.write_nrx1(0xBF);
        self.ch1.pulse.write_nrx2(0xF3);
        self.ch1.pulse.write_nrx3(0xFF);
        self.ch1.write_nr14(0xBF, false); // includes trigger (bit 7)
        // CH2 post-boot: DAC off (NR22=$00).
        self.ch2.write_nrx1(0x3F);
        self.ch2.write_nrx2(0x00);
        self.ch2.write_nrx3(0xFF);
        self.ch2.write_nrx4(0xBF, false);
        // CH3 post-boot: DAC off (NR30 bit7 clear).
        self.ch3.write_nr30(0x7F);
        self.ch3.write_nr31(0xFF);
        self.ch3.write_nr32(0x9F);
        self.ch3.write_nr33(0xFF);
        self.ch3.write_nr34(0xBF, false);
        // CH4 post-boot: DAC off (NR42=$00).
        self.ch4.write_nr41(0xFF);
        self.ch4.write_nr42(0x00);
        self.ch4.write_nr43(0x00);
        self.ch4.write_nr44(0xBF, false);
        self.mixer.write_nr50(0x77);
        self.mixer.write_nr51(0xF3);
        self.stats.reset_window();
    }

    /// Snapshot for `--verbose` diagnostics.
    pub fn debug_ch1(&self) -> Ch1Debug {
        Ch1Debug {
            nr52: self.read_nr52(),
            powered: self.powered,
            active: self.ch1.pulse.enabled(),
            dac: self.ch1.pulse.dac_enabled(),
            nr50: self.mixer.read_nr50(),
            nr51: self.mixer.read_nr51(),
            route_l: self.mixer.read_nr51() & 0x10 != 0,
            route_r: self.mixer.read_nr51() & 0x01 != 0,
            frequency: self.ch1.pulse.frequency(),
            duty: self.ch1.pulse.duty(),
            volume: self.ch1.pulse.volume(),
            samples_generated: self.stats.samples_generated,
            nonzero_samples: self.stats.nonzero_samples,
            min: self.stats.min,
            max: self.stats.max,
        }
    }

    /// Immutable channel / mixer view for the Debug Monitor.
    pub fn debug_snapshot(&self) -> crate::debug::ApuDebug {
        use crate::debug::{ApuDebug, ChannelDebug};
        ApuDebug {
            powered: self.powered,
            nr50: self.mixer.read_nr50(),
            nr51: self.mixer.read_nr51(),
            nr52: self.read_nr52(),
            ch1: ChannelDebug {
                active: self.ch1.pulse.enabled(),
                dac: self.ch1.pulse.dac_enabled(),
                volume: self.ch1.pulse.volume(),
                frequency: self.ch1.pulse.frequency(),
                duty: Some(self.ch1.pulse.duty()),
            },
            ch2: ChannelDebug {
                active: self.ch2.enabled(),
                dac: self.ch2.dac_enabled(),
                volume: self.ch2.volume(),
                frequency: self.ch2.frequency(),
                duty: Some(self.ch2.duty()),
            },
            ch3: ChannelDebug {
                active: self.ch3.enabled(),
                dac: self.ch3.dac_enabled(),
                volume: self.ch3.volume_level(),
                frequency: self.ch3.frequency(),
                duty: None,
            },
            ch4: ChannelDebug {
                active: self.ch4.enabled(),
                dac: self.ch4.dac_enabled(),
                volume: self.ch4.volume(),
                frequency: u16::from(self.ch4.read_nr43()),
                duty: None,
            },
            peak_min: if self.stats.samples_generated == 0 {
                0.0
            } else {
                self.stats.min
            },
            peak_max: if self.stats.samples_generated == 0 {
                0.0
            } else {
                self.stats.max
            },
            samples_generated: self.stats.samples_generated,
        }
    }

    pub fn sample_buffer_len(&self) -> usize {
        self.samples.len()
    }

    /// Match host device rate so we neither pace on audio nor pitch-shift.
    pub fn set_output_sample_rate(&mut self, rate: u32) {
        let rate = rate.max(1);
        self.cycles_per_sample = f64::from(CPU_HZ) / f64::from(rate);
    }

    /// Host PCM rate implied by the current T-cycle / sample ratio.
    pub fn output_sample_rate(&self) -> u32 {
        (f64::from(CPU_HZ) / self.cycles_per_sample).round() as u32
    }

    /// Drain generated stereo samples for the host audio device.
    pub fn take_samples(&mut self) -> Vec<StereoSample> {
        self.samples.drain()
    }

    /// PCM12 (`$FF76`): CH1 digital in the low nibble, CH2 in the high nibble.
    pub fn pcm12(&self) -> u8 {
        pcm::pack_pcm(self.ch1.pulse.digital_output(), self.ch2.digital_output())
    }

    /// PCM34 (`$FF77`): CH3 digital in the low nibble, CH4 in the high nibble.
    pub fn pcm34(&self) -> u8 {
        pcm::pack_pcm(self.ch3.digital_output(), self.ch4.digital_output())
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        if (WAVE_RAM_START..=WAVE_RAM_END).contains(&addr) {
            return Some(self.ch3.read_wave(addr));
        }
        match addr {
            NR10 => Some(self.ch1.read_nr10()),
            NR11 => Some(self.ch1.pulse.read_nrx1()),
            NR12 => Some(self.ch1.pulse.read_nrx2()),
            NR13 => Some(0xFF), // write-only
            NR14 => Some(self.ch1.pulse.read_nrx4()),
            0xFF15 => Some(0xFF),
            NR21 => Some(self.ch2.read_nrx1()),
            NR22 => Some(self.ch2.read_nrx2()),
            NR23 => Some(0xFF), // write-only
            NR24 => Some(self.ch2.read_nrx4()),
            NR30 => Some(self.ch3.read_nr30()),
            NR31 => Some(0xFF), // write-only
            NR32 => Some(self.ch3.read_nr32()),
            NR33 => Some(0xFF), // write-only
            NR34 => Some(self.ch3.read_nr34()),
            0xFF1F => Some(0xFF),
            NR41 => Some(0xFF), // write-only
            NR42 => Some(self.ch4.read_nr42()),
            NR43 => Some(self.ch4.read_nr43()),
            NR44 => Some(self.ch4.read_nr44()),
            NR50 => Some(self.mixer.read_nr50()),
            NR51 => Some(self.mixer.read_nr51()),
            NR52 => Some(self.read_nr52()),
            _ => None,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        // Wave RAM is accessible even while the APU is powered off.
        if (WAVE_RAM_START..=WAVE_RAM_END).contains(&addr) {
            self.ch3.write_wave(addr, value);
            return true;
        }
        if addr == NR52 {
            self.write_nr52(value);
            return true;
        }
        if !self.powered {
            // Only NR52 power bit is writable while off (already handled).
            return matches!(addr, 0xFF10..=0xFF26);
        }
        match addr {
            NR10 => {
                self.ch1.write_nr10(value);
                true
            }
            NR11 => {
                self.ch1.pulse.write_nrx1(value);
                true
            }
            NR12 => {
                self.ch1.pulse.write_nrx2(value);
                true
            }
            NR13 => {
                self.ch1.pulse.write_nrx3(value);
                true
            }
            NR14 => {
                self.ch1
                    .write_nr14(value, self.sequencer.next_frame_will_length());
                true
            }
            NR21 => {
                self.ch2.write_nrx1(value);
                true
            }
            NR22 => {
                self.ch2.write_nrx2(value);
                true
            }
            NR23 => {
                self.ch2.write_nrx3(value);
                true
            }
            NR24 => {
                self.ch2
                    .write_nrx4(value, self.sequencer.next_frame_will_length());
                true
            }
            NR30 => {
                self.ch3.write_nr30(value);
                true
            }
            NR31 => {
                self.ch3.write_nr31(value);
                true
            }
            NR32 => {
                self.ch3.write_nr32(value);
                true
            }
            NR33 => {
                self.ch3.write_nr33(value);
                true
            }
            NR34 => {
                self.ch3
                    .write_nr34(value, self.sequencer.next_frame_will_length());
                true
            }
            NR41 => {
                self.ch4.write_nr41(value);
                true
            }
            NR42 => {
                self.ch4.write_nr42(value);
                true
            }
            NR43 => {
                self.ch4.write_nr43(value);
                true
            }
            NR44 => {
                self.ch4
                    .write_nr44(value, self.sequencer.next_frame_will_length());
                true
            }
            NR50 => {
                self.mixer.write_nr50(value);
                true
            }
            NR51 => {
                self.mixer.write_nr51(value);
                true
            }
            0xFF15 | 0xFF1F => true, // unused
            _ => false,
        }
    }

    fn read_nr52(&self) -> u8 {
        let mut v = 0x70; // unused bits read as 1
        if self.powered {
            v |= 0x80;
        }
        if self.ch1.pulse.enabled() {
            v |= 0x01;
        }
        if self.ch2.enabled() {
            v |= 0x02;
        }
        if self.ch3.enabled() {
            v |= 0x04;
        }
        if self.ch4.enabled() {
            v |= 0x08;
        }
        v
    }

    fn write_nr52(&mut self, value: u8) {
        let power = value & 0x80 != 0;
        if !power && self.powered {
            self.power_off();
        } else if power && !self.powered {
            self.powered = true;
            self.sequencer.reset();
        }
        self.powered = power;
    }

    fn power_off(&mut self) {
        self.powered = false;
        self.ch1.power_off();
        self.ch2.power_off();
        self.ch3.power_off();
        self.ch4.power_off();
        self.mixer.power_off();
        self.sequencer.reset();
        self.sample_phase = 0.0;
    }

    /// Advance APU by `t_cycles` CPU clocks; may enqueue host samples.
    pub fn tick(&mut self, t_cycles: u32) {
        if t_cycles == 0 {
            return;
        }
        if !self.powered {
            // Still consume sample schedule so buffer timing stays stable when muted.
            self.enqueue_silence(t_cycles);
            return;
        }

        let mut remaining = t_cycles;
        while remaining > 0 {
            let step = remaining.min(64);
            self.sequencer.tick(step, |ev| {
                if ev.length {
                    self.ch1.pulse.clock_length();
                    self.ch2.clock_length();
                    self.ch3.clock_length();
                    self.ch4.clock_length();
                }
                if ev.envelope {
                    self.ch1.pulse.clock_envelope();
                    self.ch2.clock_envelope();
                    self.ch4.clock_envelope();
                }
                if ev.sweep {
                    self.ch1.clock_sweep();
                }
            });
            self.ch1.pulse.tick(step);
            self.ch2.tick(step);
            self.ch3.tick(step);
            self.ch4.tick(step);
            self.sample_phase += f64::from(step);
            while self.sample_phase >= self.cycles_per_sample {
                self.sample_phase -= self.cycles_per_sample;
                let (l, r) = self.mixer.mix(
                    self.ch1.pulse.dac_output(),
                    self.ch2.dac_output(),
                    self.ch3.dac_output(),
                    self.ch4.dac_output(),
                );
                let sample = StereoSample { left: l, right: r };
                self.stats.note_sample(sample);
                self.samples.push(sample);
            }
            remaining -= step;
        }
    }

    fn enqueue_silence(&mut self, t_cycles: u32) {
        self.sample_phase += f64::from(t_cycles);
        while self.sample_phase >= self.cycles_per_sample {
            self.sample_phase -= self.cycles_per_sample;
            let sample = StereoSample::SILENCE;
            self.stats.note_sample(sample);
            self.samples.push(sample);
        }
    }

    #[cfg(test)]
    pub(crate) fn frame_sequencer_step(&self) -> u8 {
        self.sequencer.step_index()
    }

    pub(crate) fn snapshot(&self) -> crate::snapshot::ApuStateV1 {
        crate::snapshot::ApuStateV1 {
            powered: self.powered,
            frame_seq_step: self.sequencer.step_index(),
            ch1: self.ch1.snapshot(),
            ch2: self.ch2.snapshot(),
            ch3: self.ch3.snapshot(),
            ch4: self.ch4.snapshot(),
            nr50: self.mixer.read_nr50(),
            nr51: self.mixer.read_nr51(),
            wave_ram: self.ch3.wave_ram_bytes(),
            sample_phase: self.sample_phase,
        }
    }

    pub(crate) fn apply_snapshot(&mut self, state: &crate::snapshot::ApuStateV1) {
        self.powered = state.powered;
        self.sequencer.set_step_index(state.frame_seq_step);
        self.ch1.apply_snapshot(&state.ch1);
        self.ch2.apply_snapshot(&state.ch2);
        self.ch3.apply_snapshot(&state.ch3, &state.wave_ram);
        self.ch4.apply_snapshot(&state.ch4);
        self.mixer.write_nr50(state.nr50);
        self.mixer.write_nr51(state.nr51);
        self.sample_phase = state.sample_phase;
    }
}

#[cfg(test)]
mod tests;

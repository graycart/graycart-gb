//! Host audio output — adaptive sync + SPSC ring → CPAL callback.
//!
//! Emulation produces PCM on the DMG clock; the host device clock drifts.
//! A PI occupancy controller (±0.5%) keeps the ring near its soft target.
//! The CPAL callback only pops samples (no locks, no emulator access).

mod init;
mod os_default;
mod resample;
mod select;

use graycart::StereoSample;
use resample::AdaptiveResampler;
use rtrb::{Consumer, Producer};
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use cpal::Stream;

pub use init::{
    AudioInitError, list_output_device_names, open_output_with_pref, probe_audio_backends,
};
pub use select::{AudioDevicePref, AudioDeviceSource, should_refresh_output_device_list};

/// Bind the emulated APU to the live host device rate after any machine rebuild.
pub fn bind_bus_host_rate(bus: &mut graycart::Bus, host_rate: Option<u32>) {
    if let Some(rate) = host_rate {
        bus.apu.set_output_sample_rate(rate);
    }
}

/// Producer/consumer counters for `--verbose` / Debug Monitor.
#[derive(Debug, Clone, Default)]
pub struct AudioStats {
    pub produced: u64,
    pub consumed: u64,
    /// Callbacks that observed an empty ring (episodes).
    pub underrun_events: u64,
    /// Stereo frames filled with silence due to empty ring.
    pub missing_samples: u64,
    pub dropped: u64,
}

pub(crate) struct SharedCounters {
    produced: AtomicU64,
    consumed: AtomicU64,
    underrun_events: AtomicU64,
    missing_samples: AtomicU64,
    dropped: AtomicU64,
    callbacks: AtomicU64,
}

impl SharedCounters {
    pub(crate) fn new() -> Self {
        Self {
            produced: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            underrun_events: AtomicU64::new(0),
            missing_samples: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> AudioStats {
        AudioStats {
            produced: self.produced.load(Ordering::Relaxed),
            consumed: self.consumed.load(Ordering::Relaxed),
            underrun_events: self.underrun_events.load(Ordering::Relaxed),
            missing_samples: self.missing_samples.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }
}

pub struct AudioOut {
    pub(crate) _stream: Stream,
    pub(crate) producer: RefCell<Producer<f32>>,
    pub(crate) consumer: Arc<Mutex<Consumer<f32>>>,
    pub(crate) capacity_samples: usize,
    pub(crate) counters: Arc<SharedCounters>,
    pub(crate) resampler: RefCell<AdaptiveResampler>,
    pub(crate) started: Instant,
    /// Device sample rate (APU should match via [`graycart::Apu::set_output_sample_rate`]).
    pub sample_rate: u32,
    /// Soft target occupancy (~3 display frames).
    pub target_frames: usize,
    pub device_name: String,
    pub host_name: String,
    pub sample_format: String,
    pub channels: u16,
    pub device_source: AudioDeviceSource,
}

impl AudioOut {
    /// Open a host output device. Errors include a backend probe dump.
    pub fn open() -> Result<Self, AudioInitError> {
        open_output_with_pref(&AudioDevicePref::SystemDefault)
    }

    pub fn open_pref(pref: &AudioDevicePref) -> Result<Self, AudioInitError> {
        open_output_with_pref(pref)
    }

    pub fn callbacks(&self) -> u64 {
        self.counters.callbacks.load(Ordering::Relaxed)
    }

    pub fn startup_line(&self) -> String {
        let role = match self.device_source {
            AudioDeviceSource::User => "user-selected",
            AudioDeviceSource::SystemDefault => "system default",
            AudioDeviceSource::Fallback => "fallback (not persisted)",
        };
        format!(
            "audio backend initialized: yes\n\
             output role: {role}\n\
             default output device: {}\n\
             host: {}\n\
             supported/default config: {}ch {} Hz {}\n\
             CPAL stream created: yes\n\
             CPAL stream.play(): success\n\
             selected host rate: {} Hz\n\
             ring capacity: {} stereo frames\n\
             ring target: {} stereo frames",
            self.device_name,
            self.host_name,
            self.channels,
            self.sample_rate,
            self.sample_format,
            self.sample_rate,
            self.capacity_samples / 2,
            self.target_frames
        )
    }

    /// Stereo frames waiting in the ring.
    pub fn queued_frames(&self) -> usize {
        let free = self.producer.borrow().slots();
        self.capacity_samples.saturating_sub(free) / 2
    }

    pub fn stats(&self) -> AudioStats {
        self.counters.snapshot()
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Last adaptive input step (1.0 = nominal).
    pub fn last_resample_step(&self) -> f64 {
        self.resampler.borrow().last_step
    }

    /// Genuine underrun risk only — not the normal sync mechanism.
    ///
    /// True when the ring is below ~25% of the soft target (or &lt; 256 frames).
    pub fn needs_emergency_catch_up(&self) -> bool {
        let q = self.queued_frames();
        let floor = (self.target_frames / 4).max(256);
        q < floor
    }

    pub fn submit(&self, samples: &[StereoSample]) {
        self.submit_gain(samples, 1.0);
    }

    /// Adaptively resample, apply gain, push into the SPSC ring.
    pub fn submit_gain(&self, samples: &[StereoSample], gain: f32) {
        if samples.is_empty() {
            return;
        }
        let queued = self.queued_frames();
        let adapted = self
            .resampler
            .borrow_mut()
            .process(samples, queued, self.target_frames);

        let gain = gain.clamp(0.0, 1.0);
        let mut producer = self.producer.borrow_mut();
        let mut produced = 0u64;
        let mut dropped = 0u64;
        for s in adapted {
            if producer.slots() < 2 {
                dropped += 1;
                continue;
            }
            // push is wait-free; slots check above should guarantee success.
            if producer.push(s.left * gain).is_err() || producer.push(s.right * gain).is_err() {
                dropped += 1;
                break;
            }
            produced += 1;
        }
        self.counters
            .produced
            .fetch_add(produced, Ordering::Relaxed);
        if dropped > 0 {
            self.counters.dropped.fetch_add(dropped, Ordering::Relaxed);
        }
    }

    /// Drop haunted PCM and reset adaptive history after load-state / rewind release.
    pub fn after_restore(&self) {
        if let Ok(mut consumer) = self.consumer.lock() {
            while consumer.pop().is_ok() {}
        }
        *self.resampler.borrow_mut() = AdaptiveResampler::new();
        let prime = (self.target_frames * 85 / 100) * 2;
        let mut producer = self.producer.borrow_mut();
        for _ in 0..prime {
            let _ = producer.push(0.0);
        }
    }

    /// One-line drift snapshot for `--verbose` (multi-minute runs).
    pub fn drift_line(&self) -> String {
        let s = self.stats();
        format!(
            "audio: t={:.1}s queued={} target={} step={:.5} produced={} consumed={} underrun_events={} missing_samples={} dropped={}",
            self.elapsed_secs(),
            self.queued_frames(),
            self.target_frames,
            self.last_resample_step(),
            s.produced,
            s.consumed,
            s.underrun_events,
            s.missing_samples,
            s.dropped
        )
    }
}

pub(crate) fn fill_f32(out: &mut [f32], consumer: &mut Consumer<f32>, counters: &SharedCounters) {
    counters.callbacks.fetch_add(1, Ordering::Relaxed);
    let mut consumed = 0u64;
    let mut missing = 0u64;
    for sample in out.iter_mut() {
        match consumer.pop() {
            Ok(v) => {
                *sample = v;
                consumed += 1;
            }
            Err(_) => {
                *sample = 0.0;
                missing += 1;
            }
        }
    }
    finish_fill(counters, consumed, missing);
}

pub(crate) fn fill_i16(out: &mut [i16], consumer: &mut Consumer<f32>, counters: &SharedCounters) {
    counters.callbacks.fetch_add(1, Ordering::Relaxed);
    let mut consumed = 0u64;
    let mut missing = 0u64;
    for sample in out.iter_mut() {
        match consumer.pop() {
            Ok(v) => {
                *sample = (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                consumed += 1;
            }
            Err(_) => {
                *sample = 0;
                missing += 1;
            }
        }
    }
    finish_fill(counters, consumed, missing);
}

pub(crate) fn fill_i32(out: &mut [i32], consumer: &mut Consumer<f32>, counters: &SharedCounters) {
    counters.callbacks.fetch_add(1, Ordering::Relaxed);
    let mut consumed = 0u64;
    let mut missing = 0u64;
    for sample in out.iter_mut() {
        match consumer.pop() {
            Ok(v) => {
                *sample = (v.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
                consumed += 1;
            }
            Err(_) => {
                *sample = 0;
                missing += 1;
            }
        }
    }
    finish_fill(counters, consumed, missing);
}

pub(crate) fn fill_u16(out: &mut [u16], consumer: &mut Consumer<f32>, counters: &SharedCounters) {
    counters.callbacks.fetch_add(1, Ordering::Relaxed);
    let mut consumed = 0u64;
    let mut missing = 0u64;
    for sample in out.iter_mut() {
        match consumer.pop() {
            Ok(v) => {
                *sample = ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * f32::from(u16::MAX)) as u16;
                consumed += 1;
            }
            Err(_) => {
                *sample = u16::MAX / 2;
                missing += 1;
            }
        }
    }
    finish_fill(counters, consumed, missing);
}

pub(crate) fn fill_f64(out: &mut [f64], consumer: &mut Consumer<f32>, counters: &SharedCounters) {
    counters.callbacks.fetch_add(1, Ordering::Relaxed);
    let mut consumed = 0u64;
    let mut missing = 0u64;
    for sample in out.iter_mut() {
        match consumer.pop() {
            Ok(v) => {
                *sample = f64::from(v);
                consumed += 1;
            }
            Err(_) => {
                *sample = 0.0;
                missing += 1;
            }
        }
    }
    finish_fill(counters, consumed, missing);
}

fn finish_fill(counters: &SharedCounters, consumed: u64, missing: u64) {
    counters.consumed.fetch_add(consumed / 2, Ordering::Relaxed);
    if missing > 0 {
        counters
            .missing_samples
            .fetch_add(missing / 2, Ordering::Relaxed);
        counters.underrun_events.fetch_add(1, Ordering::Relaxed);
    }
}

/// Direct 440 Hz square into the host device (bypasses APU/Bus).
pub fn run_tone_test(seconds: f32) -> Result<(), String> {
    let audio = AudioOut::open().map_err(|e| format!("audio-test failed:\n{e}"))?;
    println!("{}", audio.startup_line());
    let rate = audio.sample_rate;
    println!(
        "audio-test: 440 Hz square @ {rate} Hz for {seconds:.1}s (target_q={})",
        audio.target_frames
    );
    let half_period = (rate / (440 * 2)).max(1);
    let total_frames = ((rate as f32) * seconds) as u32;
    let chunk = (rate / 20).max(64);
    let mut phase = 0u32;
    let mut high = true;
    let mut played = 0u32;
    while played < total_frames {
        let n = chunk.min(total_frames - played);
        let mut buf = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let v = if high { 0.2 } else { -0.2 };
            buf.push(StereoSample { left: v, right: v });
            phase += 1;
            if phase >= half_period {
                phase = 0;
                high = !high;
            }
        }
        audio.submit(&buf);
        played += n;
        if !audio.needs_emergency_catch_up() {
            let ms = (1_000.0 * f64::from(n) / f64::from(rate)).round() as u64;
            std::thread::sleep(std::time::Duration::from_millis(ms.max(1)));
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    println!("{}", audio.drift_line());
    println!(
        "samples generated/sec: {:.0}\n\
         samples submitted/sec: {:.0}\n\
         callback invocations/sec: {:.1}",
        audio.stats().produced as f64 / audio.elapsed_secs().max(0.001),
        audio.stats().consumed as f64 / audio.elapsed_secs().max(0.001),
        audio.callbacks() as f64 / audio.elapsed_secs().max(0.001)
    );
    Ok(())
}

#[cfg(test)]
mod tests;

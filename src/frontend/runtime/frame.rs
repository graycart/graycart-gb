//! Latest-frame / latest-debug mailboxes and per-tick packets from the emulation thread.
//!
//! Publishing **never waits** on the UI: an unread value is replaced (dropped), not queued.

use super::super::playback::SpeedPreset;
use graycart::{Framebuffer, MachineDebug};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::time::Duration;

/// One completed emulation tick worth of state for the UI to present.
#[derive(Debug, Clone)]
pub struct FramePacket {
    pub framebuffer: Option<Framebuffer>,
    pub rom_loaded: bool,
    pub title: String,
    pub paused: bool,
    pub ff_toggle: bool,
    pub rewinding: bool,
    pub overlay_speed: Option<SpeedPreset>,
    pub frame_advance_flash: bool,
    pub status_toast: Option<String>,
    pub fault: Option<String>,
    pub last_frame_time: Duration,
    pub host_fps: f64,
    /// Emulation-thread measured T-cycles/sec (not UI present rate).
    pub runtime_tcycles_per_sec: f64,
    /// Emulation-thread measured emulated frames/sec.
    pub runtime_emu_fps: f64,
    /// Host presentation frames replaced because UI had not consumed them yet.
    pub frame_publish_replaced: u64,
    pub peak_l: f32,
    pub peak_r: f32,
    pub missed_frames: u64,
    pub audio_queued: Option<usize>,
    pub audio_target: Option<usize>,
    pub audio_missing: Option<u64>,
    pub audio_underrun_events: Option<u64>,
    pub audio_dropped: Option<u64>,
    pub audio_resample_step: Option<f64>,
    pub audio_sample_rate: Option<u32>,
    pub audio_produced: Option<u64>,
    pub audio_consumed: Option<u64>,
    pub audio_callbacks: Option<u64>,
    pub audio_elapsed_secs: Option<f64>,
    pub audio_device: Option<String>,
    pub audio_channels: Option<u16>,
    pub audio_buffer_size: Option<String>,
    pub audio_init_error: Option<String>,
    pub apu_ch1_debug: Option<String>,
}

impl Default for FramePacket {
    fn default() -> Self {
        Self {
            framebuffer: None,
            rom_loaded: false,
            title: String::new(),
            paused: false,
            ff_toggle: false,
            rewinding: false,
            overlay_speed: None,
            frame_advance_flash: false,
            status_toast: None,
            fault: None,
            last_frame_time: Duration::ZERO,
            host_fps: 0.0,
            runtime_tcycles_per_sec: 0.0,
            runtime_emu_fps: 0.0,
            frame_publish_replaced: 0,
            peak_l: 0.0,
            peak_r: 0.0,
            missed_frames: 0,
            audio_queued: None,
            audio_target: None,
            audio_missing: None,
            audio_underrun_events: None,
            audio_dropped: None,
            audio_resample_step: None,
            audio_sample_rate: None,
            audio_produced: None,
            audio_consumed: None,
            audio_callbacks: None,
            audio_elapsed_secs: None,
            audio_device: None,
            audio_channels: None,
            audio_buffer_size: None,
            audio_init_error: None,
            apu_ch1_debug: None,
        }
    }
}

struct SlotInner<T> {
    ptr: AtomicPtr<T>,
    replaced: AtomicU64,
}

impl<T> SlotInner<T> {
    fn new() -> Self {
        Self {
            ptr: AtomicPtr::new(ptr::null_mut()),
            replaced: AtomicU64::new(0),
        }
    }

    fn publish(&self, value: T) {
        let new = Box::into_raw(Box::new(value));
        let old = self.ptr.swap(new, Ordering::AcqRel);
        if !old.is_null() {
            // SAFETY: `old` was published by us and not taken.
            unsafe { drop(Box::from_raw(old)) };
            self.replaced.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn take(&self) -> Option<T> {
        let p = self.ptr.swap(ptr::null_mut(), Ordering::AcqRel);
        if p.is_null() {
            None
        } else {
            // SAFETY: exclusive ownership after swap to null.
            Some(*unsafe { Box::from_raw(p) })
        }
    }

    fn replaced(&self) -> u64 {
        self.replaced.load(Ordering::Relaxed)
    }
}

impl<T> Drop for SlotInner<T> {
    fn drop(&mut self) {
        let p = *self.ptr.get_mut();
        if !p.is_null() {
            // SAFETY: last owner; no concurrent accessors.
            unsafe { drop(Box::from_raw(p)) };
        }
    }
}

/// Capacity-1 latest-frame slot: publishing replaces any unread packet (lock-free).
#[derive(Clone)]
pub struct LatestFrame {
    inner: Arc<SlotInner<FramePacket>>,
}

impl std::fmt::Debug for LatestFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LatestFrame")
            .field("replaced", &self.inner.replaced())
            .finish()
    }
}

impl LatestFrame {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SlotInner::new()),
        }
    }

    pub fn publish(&self, packet: FramePacket) {
        self.inner.publish(packet);
    }

    pub fn take(&self) -> Option<FramePacket> {
        self.inner.take()
    }

    pub fn replaced_count(&self) -> u64 {
        self.inner.replaced()
    }
}

impl Default for LatestFrame {
    fn default() -> Self {
        Self::new()
    }
}

/// Capacity-1 latest machine-debug snapshot (lock-free overwrite).
#[derive(Clone)]
pub struct LatestDebug {
    inner: Arc<SlotInner<MachineDebug>>,
}

impl std::fmt::Debug for LatestDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LatestDebug")
            .field("replaced", &self.inner.replaced())
            .finish()
    }
}

impl LatestDebug {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SlotInner::new()),
        }
    }

    pub fn publish(&self, snap: MachineDebug) {
        self.inner.publish(snap);
    }

    pub fn take(&self) -> Option<MachineDebug> {
        self.inner.take()
    }
}

impl Default for LatestDebug {
    fn default() -> Self {
        Self::new()
    }
}

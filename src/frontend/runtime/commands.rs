//! Commands sent from the UI thread to the emulation thread.

use super::super::audio::AudioDevicePref;
use super::super::input::HostCommand;
use super::super::playback::SpeedPreset;
use graycart::{BootMode, Cartridge, HostHardwarePref};
use std::path::PathBuf;
use std::sync::mpsc;

/// One-way messages into the emulation thread.
pub enum EmuCommand {
    /// Stop the thread after optional save flush.
    Quit,
    /// Begin stepping; sent once after the window is ready.
    ///
    /// Host audio is opened on this thread because `cpal::Stream` is not
    /// `Send`. PCM still goes emu producer → ring → CPAL callback (not winit).
    Start,
    /// Construct a new machine from an opened ROM (UI reads the file).
    LoadRom {
        path: PathBuf,
        save_path: PathBuf,
        title: String,
        cart: Box<Cartridge>,
    },
    /// Power-on reset with the given boot mode.
    Reset {
        boot_mode: BootMode,
    },
    /// Effective joypad bitmask (see [`crate::frontend::input::button_bit`]).
    SetButtons(u8),
    HostPress(HostCommand),
    HostRelease(HostCommand),
    /// Main-window focus loss: clear transient holds; optionally pause.
    FocusLost {
        pause_when_unfocused: bool,
    },
    SetPaused(bool),
    SetFfSpeed(SpeedPreset),
    SetFfToggle(bool),
    SetRewindEnabled(bool),
    SetAudioGain(f32),
    SetTickProfiling(bool),
    /// When true, the runtime periodically publishes [`MachineDebug`] into the latest-debug mailbox.
    SetDebugPublish(bool),
    SlotSave(u8),
    SlotLoad(u8),
    FrameAdvance,
    /// Flush battery `.sav` to disk; reply when done (shutdown / rare UI path only).
    FlushSave {
        reply: mpsc::Sender<Result<(), String>>,
    },
    /// Host hardware preference. Reloads the current ROM through
    /// [`graycart::bus_from_cartridge`] so silicon matches the pref.
    SetHardwarePref(HostHardwarePref),
    /// Re-open the CPAL stream for a new output preference (emu thread owns Stream).
    SetAudioOutput(AudioDevicePref),
}

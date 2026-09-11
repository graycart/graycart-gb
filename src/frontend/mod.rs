//! Thin display frontend: framebuffer in → window out; host keys → GameBoyButton;
//! APU PCM → host audio; egui menus for host settings.
//!
//! No tiles, LY, VRAM, or interrupt knowledge — only shade pixels,
//! optional wall-clock pacing, keyboard edge mapping, and sample playback.

mod app;
mod audio;
mod boot_rom;
mod brand;
mod controls;
mod debug;
mod fonts;
mod host;
mod input;
mod launch;
mod pace;
mod playback;
mod report;
mod rewind;
mod rom;
mod runtime;
mod screenshot;
mod settings;
mod state_slots;
mod telemetry;
mod ui;
mod video;

pub use app::run;
pub use audio::run_tone_test;
pub use launch::{LaunchRom, Verbosity};
pub use rom::is_rom_path;

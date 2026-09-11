//! Game Boy joypad hardware (host-agnostic).
//!
//! Host keyboard/gamepad IDs live in `frontend/` and map into [`GameBoyButton`].

mod button;
mod joypad;

pub use button::GameBoyButton;
pub use joypad::{Joypad, P1};

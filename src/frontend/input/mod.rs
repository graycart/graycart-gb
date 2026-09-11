//! Host keyboard + gamepad → [`GameBoyButton`] (no `$FF00` / IF knowledge).

mod combine;
mod commands;
mod gamepad;
mod listen;
mod mapping;
mod stick;

use super::settings::FrontendSettings;
use graycart::GameBoyButton;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[allow(unused_imports)] // exported for the input configuration UI
pub use super::settings::{GamepadProfile, InputSettings};
pub use combine::{EdgeTracker, InputEdges, button_bit, combine};
pub use commands::{HostCommand, HostCommandMap, is_reserved_host_key};
#[allow(unused_imports)] // exported for the input configuration UI
pub use gamepad::{ConnectedPad, GamepadFrame, GamepadHub, pad_button_from_gilrs};
pub use listen::{ListenState, ListenTarget};
pub use mapping::{GamepadMap, KeyboardMap, PadButton, keycode_label, pad_button_label};
pub use stick::StickDigital;

pub struct InputFrontend {
    hub: GamepadHub,
    edges: EdgeTracker,
    listen: ListenState,
}

#[derive(Default)]
pub struct PollResult {
    #[allow(dead_code)] // retained for future edge-triggered host actions
    pub edges: InputEdges,
    pub effective_mask: u8,
    pub frame: GamepadFrame,
}

impl Default for InputFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputFrontend {
    pub fn new() -> Self {
        Self {
            hub: GamepadHub::new(),
            edges: EdgeTracker::default(),
            listen: ListenState::default(),
        }
    }

    pub fn begin_listen(&mut self, target: ListenTarget) {
        self.listen.begin(target);
    }

    pub fn cancel(&mut self) {
        self.listen.cancel();
    }

    pub fn reset_edges(&mut self) {
        self.edges.reset();
    }

    pub fn is_listening(&self) -> bool {
        self.listen.is_active()
    }

    /// Routes a physical key to an active keyboard listen.
    ///
    /// Returns true for a commit or Escape cancellation.
    pub fn on_listen_key(
        &mut self,
        key: KeyCode,
        map: &mut KeyboardMap,
        host: &HostCommandMap,
    ) -> bool {
        self.listen.on_key(key, map, host)
    }

    /// Poll all host input even when keyboard gameplay is temporarily routed to UI.
    ///
    /// When `track_edges` is false, the effective mask is still computed but
    /// [`EdgeTracker`] is not advanced — the next tracked poll emits presses for
    /// keys already held (e.g. first frame after ROM load).
    pub fn poll(
        &mut self,
        settings: &mut FrontendSettings,
        keys_down: &HashSet<KeyCode>,
        keyboard_gameplay: bool,
        track_edges: bool,
    ) -> PollResult {
        let listening = self.listen.is_active();
        let mut frame = self.hub.pump(&mut settings.input);
        if matches!(self.listen.active, Some(ListenTarget::Gamepad(_)))
            && let Some(profile_id) = frame.active_profile_id.as_deref()
            && let Some(profile) = settings.input.profile_mut(profile_id)
            && frame
                .pressed_buttons
                .iter()
                .copied()
                .any(|pad| self.listen.on_pad_button(pad, &mut profile.map))
        {
            frame.settings_dirty = true;
        }
        let keyboard = if keyboard_gameplay && !listening {
            keyboard_mask(&settings.input.keyboard, keys_down)
        } else {
            0
        };
        let effective_mask = combine(keyboard, frame.pad_mask);
        let edges = if listening || !track_edges {
            InputEdges::default()
        } else {
            self.edges.edges(effective_mask)
        };
        PollResult {
            edges,
            effective_mask,
            frame,
        }
    }
}

fn keyboard_mask(map: &KeyboardMap, keys_down: &HashSet<KeyCode>) -> u8 {
    let mut mask = 0;
    for button in GameBoyButton::ALL {
        if keys_down.contains(&map.key(button)) {
            mask |= button_bit(button);
        }
    }
    mask
}

#[cfg(test)]
mod tests;

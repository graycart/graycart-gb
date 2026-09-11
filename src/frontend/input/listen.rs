//! One-shot keyboard and gamepad binding capture.

use super::{GamepadMap, HostCommandMap, KeyboardMap, PadButton, is_reserved_host_key};
use graycart::GameBoyButton;
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // constructed by the input configuration UI in Task 7
pub enum ListenTarget {
    Keyboard(GameBoyButton),
    Gamepad(GameBoyButton),
}

#[derive(Debug, Default)]
pub struct ListenState {
    pub active: Option<ListenTarget>,
}

impl ListenState {
    #[allow(dead_code)] // called through InputFrontend by the Task 7 UI
    pub fn begin(&mut self, target: ListenTarget) {
        self.active = Some(target);
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Returns true when the key committed a binding or cancelled listening.
    pub fn on_key(&mut self, key: KeyCode, map: &mut KeyboardMap, host: &HostCommandMap) -> bool {
        if self.active.is_none() {
            return false;
        }
        if key == KeyCode::Escape {
            self.cancel();
            return true;
        }
        let Some(ListenTarget::Keyboard(button)) = self.active else {
            return false;
        };
        if key == KeyCode::F9
            || is_reserved_host_key(key, host)
            || super::mapping::keycode_from_name(&format!("{key:?}")).is_none()
        {
            return false;
        }
        map.bind(button, key);
        self.cancel();
        true
    }

    /// Returns true when the button committed a binding.
    pub fn on_pad_button(&mut self, pad: PadButton, map: &mut GamepadMap) -> bool {
        let Some(ListenTarget::Gamepad(button)) = self.active else {
            return false;
        };
        map.bind(button, pad);
        self.cancel();
        true
    }
}

#[cfg(test)]
mod tests;

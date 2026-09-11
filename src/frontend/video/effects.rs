//! Frontend-only display modes applied after shade→RGB (never touches core FB).

use graycart::{SCREEN_HEIGHT, SCREEN_WIDTH};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Sharp,
    SoftLcd,
    Soft,
    PixelGrid,
    DmgGhosting,
}

impl DisplayMode {
    pub const ALL: [Self; 5] = [
        Self::Sharp,
        Self::SoftLcd,
        Self::Soft,
        Self::PixelGrid,
        Self::DmgGhosting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sharp => "Sharp Pixels",
            Self::SoftLcd => "Soft LCD",
            Self::Soft => "Soft",
            Self::PixelGrid => "Pixel Grid",
            Self::DmgGhosting => "DMG Ghosting",
        }
    }

    /// Prefer linear filtering (Fill) vs nearest (PixelPerfect).
    pub fn prefers_linear_filter(self) -> bool {
        matches!(self, Self::SoftLcd | Self::Soft)
    }
}

/// Apply a CPU-side presentation effect in-place on RGBA8888 `frame`.
///
/// `previous` holds the last presented frame for ghosting (updated here).
pub fn apply_effect(mode: DisplayMode, frame: &mut [u8], previous: &mut Option<Vec<u8>>) {
    debug_assert_eq!(frame.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    match mode {
        DisplayMode::Sharp | DisplayMode::SoftLcd => {
            *previous = Some(frame.to_vec());
        }
        DisplayMode::Soft => {
            soft_blur(frame);
            *previous = Some(frame.to_vec());
        }
        DisplayMode::PixelGrid => {
            pixel_grid(frame);
            *previous = Some(frame.to_vec());
        }
        DisplayMode::DmgGhosting => {
            if let Some(prev) = previous.as_ref() {
                ghost_blend(frame, prev, 0.28);
            }
            *previous = Some(frame.to_vec());
        }
    }
}

fn soft_blur(frame: &mut [u8]) {
    let src = frame.to_vec();
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= SCREEN_WIDTH as i32 || ny >= SCREEN_HEIGHT as i32 {
                        continue;
                    }
                    let i = ((ny as usize) * SCREEN_WIDTH + nx as usize) * 4;
                    acc[0] += u32::from(src[i]);
                    acc[1] += u32::from(src[i + 1]);
                    acc[2] += u32::from(src[i + 2]);
                    n += 1;
                }
            }
            let o = (y * SCREEN_WIDTH + x) * 4;
            frame[o] = (acc[0] / n) as u8;
            frame[o + 1] = (acc[1] / n) as u8;
            frame[o + 2] = (acc[2] / n) as u8;
        }
    }
}

fn pixel_grid(frame: &mut [u8]) {
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if x % 2 == 1 || y % 2 == 1 {
                let o = (y * SCREEN_WIDTH + x) * 4;
                frame[o] = (u16::from(frame[o]) * 3 / 4) as u8;
                frame[o + 1] = (u16::from(frame[o + 1]) * 3 / 4) as u8;
                frame[o + 2] = (u16::from(frame[o + 2]) * 3 / 4) as u8;
            }
        }
    }
}

fn ghost_blend(frame: &mut [u8], previous: &[u8], ghost: f32) {
    let keep = 1.0 - ghost;
    for (dst, &src) in frame.iter_mut().zip(previous.iter()) {
        *dst = (f32::from(*dst) * keep + f32::from(src) * ghost) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghosting_changes_frame_when_history_exists() {
        let mut frame = vec![255u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        let mut prev = Some(vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4]);
        apply_effect(DisplayMode::DmgGhosting, &mut frame, &mut prev);
        assert!(frame[0] < 255);
    }
}

//! Game viewport layout — scale 160×144 into the content rect below the menu bar.

use graycart::{SCREEN_HEIGHT, SCREEN_WIDTH};

use super::DisplayMode;

/// Compute the on-screen size for the GB framebuffer inside `available` (logical points).
///
/// Uses the same rules as pixels `PixelPerfect` / `Fill`: integer scale when requested
/// and the mode prefers nearest filtering; otherwise aspect-preserving fractional fit.
pub fn game_image_size(
    available_w: f32,
    available_h: f32,
    integer_scaling: bool,
    mode: DisplayMode,
) -> (f32, f32) {
    let tw = SCREEN_WIDTH as f32;
    let th = SCREEN_HEIGHT as f32;
    if available_w <= 0.0 || available_h <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = if integer_scaling && !mode.prefers_linear_filter() {
        (available_w / tw).min(available_h / th).floor().max(1.0)
    } else {
        (available_w / tw).min(available_h / th).max(0.0)
    };
    (tw * scale, th * scale)
}

#[cfg(test)]
mod tests;

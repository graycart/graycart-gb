//! Graycart chrome theme — dark high-contrast egui Visuals matching the UI kit.
//!
//! Procedural style only (no 9-slice atlas). Shared by main window, Controls, and Monitor.

use egui::style::{WidgetVisuals, Widgets};
use egui::{Color32, Context, CornerRadius, Shadow, Stroke, Style, Visuals};

/// Mint accent from the Graycart UI kit reference (`#76d6a0`).
pub const ACCENT_MINT: Color32 = Color32::from_rgb(118, 214, 160);

/// Deep panel fill (slightly blue-charcoal, kit-adjacent).
pub const PANEL_FILL: Color32 = Color32::from_rgb(22, 24, 32);

/// Window / popup fill — a step above panel.
pub const WINDOW_FILL: Color32 = Color32::from_rgb(28, 30, 40);

/// Recessed wells (text edits, canvas letterbox neighbor).
pub const EXTREME_BG: Color32 = Color32::from_rgb(12, 14, 18);

/// Default text / icon stroke.
pub const TEXT: Color32 = Color32::from_rgb(220, 224, 230);

/// Bevel-ish widget border.
pub const BORDER: Color32 = Color32::from_rgb(70, 74, 88);

/// Hovered border highlight.
pub const BORDER_HI: Color32 = Color32::from_rgb(100, 108, 124);

/// Inactive toolbar / button fill.
pub const BUTTON_FILL: Color32 = Color32::from_rgb(40, 44, 56);

/// Hovered button fill.
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(52, 58, 72);

/// Pressed / active fill.
pub const BUTTON_ACTIVE: Color32 = Color32::from_rgb(32, 36, 46);

/// Apply Graycart dark chrome to an egui [`Context`] (spacing + visuals).
pub fn apply_graycart_theme(ctx: &Context) {
    let mut style = Style {
        visuals: graycart_visuals(),
        ..(*ctx.global_style()).clone()
    };
    style.spacing.item_spacing = egui::vec2(4.0, 2.0);
    style.spacing.button_padding = egui::vec2(5.0, 2.0);
    style.spacing.interact_size = egui::vec2(36.0, 18.0);
    style.spacing.indent = 12.0;
    style.spacing.window_margin = egui::Margin::same(6);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.spacing.combo_width = 120.0;
    ctx.set_global_style(style);
}

/// Dark high-contrast visuals with mint selection and tight widget radii.
pub fn graycart_visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_alpha = 0.55;
    visuals.widgets = graycart_widgets();
    visuals.selection.bg_fill = Color32::from_rgb(36, 84, 64);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_MINT);
    visuals.hyperlink_color = ACCENT_MINT;
    visuals.faint_bg_color = Color32::from_rgb(30, 34, 44);
    visuals.extreme_bg_color = EXTREME_BG;
    visuals.text_edit_bg_color = Some(EXTREME_BG);
    visuals.code_bg_color = Color32::from_rgb(36, 40, 52);
    visuals.warn_fg_color = Color32::from_rgb(232, 168, 72);
    visuals.error_fg_color = Color32::from_rgb(220, 80, 80);
    visuals.window_corner_radius = CornerRadius::same(3);
    visuals.window_shadow = Shadow::NONE;
    visuals.window_fill = WINDOW_FILL;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.menu_corner_radius = CornerRadius::same(2);
    visuals.panel_fill = PANEL_FILL;
    visuals.popup_shadow = Shadow::NONE;
    visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.7 };
    visuals.slider_trailing_fill = true;
    visuals
}

fn graycart_widgets() -> Widgets {
    let radius = CornerRadius::same(2);
    Widgets {
        noninteractive: WidgetVisuals {
            weak_bg_fill: PANEL_FILL,
            bg_fill: PANEL_FILL,
            bg_stroke: Stroke::new(1.0_f32, BORDER),
            fg_stroke: Stroke::new(1.0_f32, Color32::from_rgb(160, 166, 178)),
            corner_radius: radius,
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            weak_bg_fill: BUTTON_FILL,
            bg_fill: BUTTON_FILL,
            bg_stroke: Stroke::new(1.0_f32, BORDER),
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            corner_radius: radius,
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            weak_bg_fill: BUTTON_HOVER,
            bg_fill: BUTTON_HOVER,
            bg_stroke: Stroke::new(1.0_f32, BORDER_HI),
            fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
            corner_radius: radius,
            expansion: 0.0,
        },
        active: WidgetVisuals {
            weak_bg_fill: BUTTON_ACTIVE,
            bg_fill: BUTTON_ACTIVE,
            bg_stroke: Stroke::new(1.0_f32, ACCENT_MINT),
            fg_stroke: Stroke::new(1.0_f32, ACCENT_MINT),
            corner_radius: radius,
            expansion: 0.0,
        },
        open: WidgetVisuals {
            weak_bg_fill: BUTTON_HOVER,
            bg_fill: BUTTON_HOVER,
            bg_stroke: Stroke::new(1.0_f32, ACCENT_MINT),
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            corner_radius: radius,
            expansion: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_accent_matches_ui_kit() {
        assert_eq!(ACCENT_MINT, Color32::from_rgb(0x76, 0xd6, 0xa0));
    }

    #[test]
    fn dark_visuals_use_panel_fill() {
        let v = graycart_visuals();
        assert_eq!(v.panel_fill, PANEL_FILL);
        assert_eq!(v.window_fill, WINDOW_FILL);
        assert!(v.dark_mode);
    }
}

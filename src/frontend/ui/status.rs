//! Bottom status bar — ROM title, hardware pref, pause/speed, FPS, toast.

use super::super::playback::SpeedPreset;
use super::menu::RuntimeUi;
use super::theme::{ACCENT_MINT, TEXT};
use egui::{Color32, RichText, Ui};
use graycart::HostHardwarePref;

/// Persistent bottom chrome. Toast replaces the idle “Ready” label when present.
pub fn status_bar(
    ui: &mut Ui,
    runtime: &RuntimeUi,
    hardware_pref: HostHardwarePref,
    ff_speed: SpeedPreset,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
        ui.add_space(4.0);

        if let Some(msg) = &runtime.status_toast {
            status_dot(ui, Color32::from_rgb(232, 168, 72));
            ui.label(RichText::new(msg).color(TEXT).size(12.0));
        } else {
            status_dot(ui, ACCENT_MINT);
            ui.label(RichText::new("Ready").color(TEXT).size(12.0));
        }

        ui.separator();

        let title = if runtime.rom_title.is_empty() {
            if runtime.rom_loaded {
                "(untitled)"
            } else {
                "No ROM"
            }
        } else {
            runtime.rom_title.as_str()
        };
        ui.label(RichText::new(title).color(TEXT).size(12.0));

        ui.separator();

        ui.label(
            RichText::new(hardware_pref.menu_label())
                .color(Color32::from_rgb(170, 178, 190))
                .size(12.0),
        );

        ui.separator();

        ui.label(
            RichText::new(playback_label(runtime, ff_speed))
                .color(TEXT)
                .size(12.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            let fps = if runtime.host_fps > 0.0 {
                format!("{:.0} FPS", runtime.host_fps)
            } else if runtime.emu_fps > 0.0 {
                format!("{:.0} FPS", runtime.emu_fps)
            } else {
                "— FPS".to_string()
            };
            ui.label(
                RichText::new(fps)
                    .color(Color32::from_rgb(170, 178, 190))
                    .size(12.0),
            );
            if runtime.emu_fps > 0.0 && runtime.host_fps > 0.0 {
                ui.label(
                    RichText::new(format!("emu {:.0}", runtime.emu_fps))
                        .color(Color32::from_rgb(120, 128, 140))
                        .size(11.0),
                );
            }
        });
    });
}

fn playback_label(runtime: &RuntimeUi, ff_speed: SpeedPreset) -> String {
    if !runtime.rom_loaded {
        return "Idle".to_string();
    }
    if runtime.overlay_rewinding {
        return "Rewind".to_string();
    }
    if runtime.overlay_frame_advance {
        return "Frame +1".to_string();
    }
    if runtime.paused || runtime.overlay_paused {
        return "Paused".to_string();
    }
    if runtime.ff_toggle {
        return format!("FF {}", ff_speed.label());
    }
    if let Some(speed) = runtime.overlay_speed.filter(|s| *s != SpeedPreset::X1) {
        return format!("Speed {}", speed.label());
    }
    "Playing".to_string()
}

fn status_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

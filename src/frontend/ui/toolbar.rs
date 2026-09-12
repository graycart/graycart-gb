//! Compact toolbar strip under the menu — working actions only.

use super::menu::{RuntimeUi, UiAction};
use super::theme::{
    ACCENT_MINT, BORDER, BORDER_HI, BUTTON_ACTIVE, BUTTON_FILL, BUTTON_HOVER, TEXT,
};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2, vec2};

const BTN: Vec2 = vec2(24.0, 24.0);

#[derive(Clone, Copy)]
enum Icon {
    Open,
    Play,
    Pause,
    Reset,
    FastForward,
    Screenshot,
    Controls,
    Monitor,
}

/// Toolbar strip: Open, Pause/Play, Reset, FF, Screenshot, Controls, Monitor.
pub fn toolbar(ui: &mut Ui, runtime: &mut RuntimeUi, actions: &mut Vec<UiAction>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
        ui.add_space(2.0);

        if tool_btn(ui, "Open ROM", false, true, Icon::Open).clicked() {
            actions.push(UiAction::OpenRomDialog);
        }

        ui.add_space(4.0);
        separator_tick(ui);

        let rom = runtime.rom_loaded;
        let pause_tip = if runtime.paused { "Resume" } else { "Pause" };
        let pause_icon = if runtime.paused {
            Icon::Play
        } else {
            Icon::Pause
        };
        if tool_btn(ui, pause_tip, runtime.paused && rom, rom, pause_icon).clicked() {
            actions.push(UiAction::TogglePause);
        }
        if tool_btn(ui, "Reset", false, rom, Icon::Reset).clicked() {
            actions.push(UiAction::Reset);
        }
        if tool_btn(
            ui,
            "Fast Forward",
            runtime.ff_toggle && rom,
            rom,
            Icon::FastForward,
        )
        .clicked()
        {
            runtime.ff_toggle = !runtime.ff_toggle;
            actions.push(UiAction::SetFfToggle(runtime.ff_toggle));
        }
        if tool_btn(ui, "Screenshot", false, rom, Icon::Screenshot).clicked() {
            actions.push(UiAction::TakeScreenshot);
        }

        ui.add_space(4.0);
        separator_tick(ui);

        if tool_btn(
            ui,
            if runtime.controls_open {
                "Close Controls"
            } else {
                "Configure Controls"
            },
            runtime.controls_open,
            true,
            Icon::Controls,
        )
        .clicked()
        {
            actions.push(UiAction::ToggleControls);
        }
        if tool_btn(
            ui,
            if runtime.debug_monitor_open {
                "Close Monitor"
            } else {
                "Open Monitor"
            },
            runtime.debug_monitor_open,
            true,
            Icon::Monitor,
        )
        .clicked()
        {
            actions.push(UiAction::ToggleDebugMonitor);
        }
    });
}

fn separator_tick(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(4.0, BTN.y), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [
            Pos2::new(x, rect.top() + 4.0),
            Pos2::new(x, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0_f32, BORDER),
    );
}

fn tool_btn(ui: &mut Ui, tip: &str, selected: bool, enabled: bool, icon: Icon) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(BTN, sense);
    let response = response.on_hover_text(tip);
    paint_button(
        ui,
        rect,
        selected,
        enabled,
        response.hovered(),
        enabled && response.is_pointer_button_down_on(),
        icon,
    );
    response
}

fn paint_button(
    ui: &Ui,
    rect: Rect,
    selected: bool,
    enabled: bool,
    hovered: bool,
    pressed: bool,
    icon: Icon,
) {
    let fill = if !enabled {
        Color32::from_rgb(28, 30, 38)
    } else if pressed {
        BUTTON_ACTIVE
    } else if selected {
        Color32::from_rgb(36, 70, 56)
    } else if hovered {
        BUTTON_HOVER
    } else {
        BUTTON_FILL
    };
    let stroke_color = if !enabled {
        Color32::from_rgb(48, 52, 64)
    } else if selected || pressed {
        ACCENT_MINT
    } else if hovered {
        BORDER_HI
    } else {
        BORDER
    };
    let icon_color = if !enabled {
        Color32::from_rgb(90, 96, 110)
    } else if selected {
        ACCENT_MINT
    } else {
        TEXT
    };

    ui.painter().rect(
        rect,
        2.0,
        fill,
        Stroke::new(1.0_f32, stroke_color),
        egui::StrokeKind::Inside,
    );
    if enabled && !pressed {
        let painter = ui.painter();
        painter.line_segment(
            [
                Pos2::new(rect.left() + 1.0, rect.top() + 1.0),
                Pos2::new(rect.right() - 1.0, rect.top() + 1.0),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(22)),
        );
        painter.line_segment(
            [
                Pos2::new(rect.left() + 1.0, rect.top() + 1.0),
                Pos2::new(rect.left() + 1.0, rect.bottom() - 1.0),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(18)),
        );
    }

    paint_icon(ui, rect.shrink(5.0), icon, icon_color);
}

fn paint_icon(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let p = ui.painter();
    let stroke = Stroke::new(1.25_f32, color);
    let c = rect.center();
    match icon {
        Icon::Open => {
            p.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(rect.left() + 1.0, rect.top() + 2.0),
                    Pos2::new(rect.left() + 7.0, rect.top() + 5.0),
                ),
                0.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(rect.left() + 1.0, rect.top() + 4.5),
                    Pos2::new(rect.right() - 1.0, rect.bottom() - 1.0),
                ),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Play => {
            let pts = [
                Pos2::new(rect.left() + 3.0, rect.top() + 1.0),
                Pos2::new(rect.right() - 1.0, c.y),
                Pos2::new(rect.left() + 3.0, rect.bottom() - 1.0),
            ];
            p.add(egui::Shape::convex_polygon(
                pts.to_vec(),
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            let w = 2.5;
            let gap = 2.0;
            let h = rect.height() - 2.0;
            p.rect_filled(
                Rect::from_center_size(Pos2::new(c.x - gap - w / 2.0, c.y), vec2(w, h)),
                0.5,
                color,
            );
            p.rect_filled(
                Rect::from_center_size(Pos2::new(c.x + gap + w / 2.0, c.y), vec2(w, h)),
                0.5,
                color,
            );
        }
        Icon::Reset => {
            // Open ring with an arrow tip (procedural reset glyph).
            let r = rect.width() * 0.40;
            let steps = 20;
            let start = std::f32::consts::FRAC_PI_4;
            let end = std::f32::consts::TAU - 0.35;
            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let a = start + (end - start) * t;
                pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
            }
            p.add(egui::Shape::line(pts, stroke));
            let tip = Pos2::new(c.x + r * start.cos(), c.y + r * start.sin());
            p.line_segment([tip, Pos2::new(tip.x + 3.0, tip.y + 0.5)], stroke);
            p.line_segment([tip, Pos2::new(tip.x + 0.5, tip.y + 3.0)], stroke);
        }
        Icon::FastForward => {
            let tri = |ox: f32| {
                [
                    Pos2::new(rect.left() + ox, rect.top() + 1.5),
                    Pos2::new(rect.left() + ox + 6.0, c.y),
                    Pos2::new(rect.left() + ox, rect.bottom() - 1.5),
                ]
            };
            p.add(egui::Shape::convex_polygon(
                tri(1.0).to_vec(),
                color,
                Stroke::NONE,
            ));
            p.add(egui::Shape::convex_polygon(
                tri(7.0).to_vec(),
                color,
                Stroke::NONE,
            ));
        }
        Icon::Screenshot => {
            p.rect_stroke(rect.shrink(1.0), 1.0, stroke, egui::StrokeKind::Inside);
            p.circle_stroke(c, 2.8, stroke);
            p.rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.right() - 5.5, rect.top() + 1.5),
                    vec2(3.5, 2.0),
                ),
                0.5,
                color,
            );
        }
        Icon::Controls => {
            let arm = 2.2;
            let len = rect.width() * 0.55;
            p.rect_filled(Rect::from_center_size(c, vec2(len, arm)), 0.5, color);
            p.rect_filled(Rect::from_center_size(c, vec2(arm, len)), 0.5, color);
        }
        Icon::Monitor => {
            let base = rect.bottom() - 1.0;
            let w = 2.5;
            let gaps = 2.0;
            let x0 = rect.left() + 1.5;
            for (i, h) in [0.35_f32, 0.7, 0.5].iter().enumerate() {
                let x = x0 + i as f32 * (w + gaps);
                let height = rect.height() * *h;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(x, base - height), Pos2::new(x + w, base)),
                    0.5,
                    color,
                );
            }
        }
    }
}

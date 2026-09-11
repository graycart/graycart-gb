//! Dense, scrollable Machine Monitor UI (btop-inspired).

use super::super::settings::MonitorSections;
use super::history::{HistoryBuffers, Sparkline};
use super::host::HostMetrics;
use egui::{CollapsingHeader, Color32, FontFamily, FontId, RichText, Sense, Stroke, Ui, Vec2};
use graycart::{GameBoyButton, MachineDebug};

pub struct DebugFrame<'a> {
    pub machine: Option<&'a MachineDebug>,
    pub host: &'a HostMetrics,
    pub history: &'a HistoryBuffers,
    pub capture_pending: bool,
    pub capture_remaining_secs: Option<f32>,
    pub has_report: bool,
    pub last_report: Option<&'a str>,
    pub sections: &'a mut MonitorSections,
    pub show_report: &'a mut bool,
    pub profile_show_max: &'a mut bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorAction {
    Capture,
    CopyReport,
    SaveReport,
    CopySnapshot,
    ViewReport,
}

pub fn draw_monitor(ui: &mut Ui, frame: &mut DebugFrame<'_>) -> Option<MonitorAction> {
    let mut action = None;
    let host = frame.host;

    // ── Fixed header ──────────────────────────────────────────────
    draw_status_banner(ui, host);
    draw_title_row(ui, frame);
    ui.add_space(2.0);
    action = action.or(draw_capture_toolbar(ui, frame));

    if *frame.show_report
        && let Some(report_action) = draw_report_viewer(ui, frame)
    {
        action = action.or(Some(report_action));
    }

    ui.add_space(4.0);
    ui.separator();

    // ── Scrollable body ───────────────────────────────────────────
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            draw_health_overview(ui, frame);
            ui.add_space(6.0);
            draw_graphs(ui, frame);
            ui.add_space(8.0);

            let width = ui.available_width();
            let cols = column_count(width);
            let panels = visible_panels(frame);
            draw_panel_grid(ui, frame, &panels, cols);
        });

    action
}

fn draw_status_banner(ui: &mut Ui, host: &HostMetrics) {
    #[cfg(debug_assertions)]
    {
        let _ = host;
        ui.colored_label(
            Color32::from_rgb(220, 160, 40),
            RichText::new("BUILD DEBUG  ·  not representative of play performance")
                .font(FontId::monospace(12.0))
                .strong(),
        );
    }
    #[cfg(not(debug_assertions))]
    {
        let rt = host.realtime_pct();
        let (mark, color) = if rt >= 99.0 {
            ("OK  ", Color32::from_rgb(80, 220, 120))
        } else if rt >= 97.0 {
            ("WARN", Color32::from_rgb(220, 180, 60))
        } else {
            ("BAD ", Color32::from_rgb(220, 80, 80))
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("BUILD RELEASE")
                    .font(FontId::monospace(12.0))
                    .color(Color32::from_rgb(160, 200, 180))
                    .strong(),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("REALTIME {}", fmt_overview_pct(rt)))
                    .font(FontId::monospace(12.0))
                    .color(color)
                    .strong(),
            );
            ui.label(
                RichText::new(mark)
                    .font(FontId::monospace(12.0))
                    .color(color),
            );
            let audio = audio_health(host);
            ui.separator();
            ui.label(
                RichText::new(format!("AUDIO {}", audio.label))
                    .font(FontId::monospace(12.0))
                    .color(health_color(audio.level)),
            );
        });
    }
}

fn draw_title_row(ui: &mut Ui, frame: &DebugFrame<'_>) {
    let host = frame.host;
    let running = frame.machine.is_some();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Graycart // Machine Monitor")
                .font(FontId::new(13.0, FontFamily::Monospace))
                .color(Color32::from_rgb(180, 220, 200))
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:>5.1} HOST", host.host_fps.clamp(0.0, 999.9)))
                    .font(FontId::monospace(11.0))
                    .color(Color32::DARK_GRAY),
            );
        });
    });
    ui.horizontal(|ui| {
        let (dot, label, color) = if running {
            (
                Color32::from_rgb(80, 220, 120),
                "RUNNING",
                Color32::from_rgb(80, 220, 120),
            )
        } else {
            (Color32::DARK_GRAY, "IDLE", Color32::GRAY)
        };
        ui.colored_label(dot, "●");
        ui.label(
            RichText::new(label)
                .font(FontId::monospace(12.0))
                .color(color)
                .strong(),
        );
        ui.separator();
        ui.monospace(if host.rom_title.is_empty() {
            "—"
        } else {
            host.rom_title.as_str()
        });
        ui.separator();
        ui.label(
            RichText::new("DMG")
                .font(FontId::monospace(11.0))
                .color(Color32::from_rgb(100, 140, 180)),
        );
    });
}

fn draw_capture_toolbar(ui: &mut Ui, frame: &mut DebugFrame<'_>) -> Option<MonitorAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let cap_label = if frame.capture_pending {
            "CAPTURING…"
        } else {
            "CAPTURE  F9"
        };
        if ui
            .add_enabled(
                !frame.capture_pending,
                egui::Button::new(RichText::new(cap_label).monospace()),
            )
            .on_hover_text("History capture: −15s … +5s around trigger")
            .clicked()
        {
            action = Some(MonitorAction::Capture);
        }
        if ui
            .button(RichText::new("COPY SNAPSHOT").monospace())
            .on_hover_text("Immediate current-state snapshot (no history)")
            .clicked()
        {
            action = Some(MonitorAction::CopySnapshot);
        }

        if frame.has_report {
            ui.separator();
            ui.label(
                RichText::new("CAPTURE READY")
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(80, 220, 120))
                    .strong(),
            );
            if ui.button(RichText::new("COPY").monospace()).clicked() {
                action = Some(MonitorAction::CopyReport);
            }
            if ui.button(RichText::new("SAVE…").monospace()).clicked() {
                action = Some(MonitorAction::SaveReport);
            }
            if ui.button(RichText::new("VIEW").monospace()).clicked() {
                *frame.show_report = !*frame.show_report;
                action = Some(MonitorAction::ViewReport);
            }
        }
    });

    if frame.capture_pending
        && let Some(rem) = frame.capture_remaining_secs
    {
        let total = 5.0_f32;
        let done = ((total - rem) / total).clamp(0.0, 1.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("CAPTURING")
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(220, 180, 60)),
            );
            bar(ui, done, Color32::from_rgb(220, 180, 60), 160.0);
            ui.label(
                RichText::new(format!("{rem:.1}s remaining"))
                    .font(FontId::monospace(11.0))
                    .color(Color32::GRAY),
            );
        });
    }
    action
}

fn draw_report_viewer(ui: &mut Ui, frame: &mut DebugFrame<'_>) -> Option<MonitorAction> {
    let mut action = None;
    let Some(report) = frame.last_report else {
        *frame.show_report = false;
        return None;
    };
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(18, 20, 22))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Last capture report")
                        .font(FontId::proportional(13.0))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Close").clicked() {
                        *frame.show_report = false;
                    }
                    if ui.small_button("Copy").clicked() {
                        action = Some(MonitorAction::CopyReport);
                    }
                });
            });
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(report)
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(200, 200, 200)),
                    );
                });
        });
    action
}

fn draw_health_overview(ui: &mut Ui, frame: &DebugFrame<'_>) {
    let host = frame.host;
    let rt = host.realtime_pct();
    let audio = audio_health(host);

    card(ui, "OVERVIEW", |ui| {
        let avail = ui.available_width().max(1.0);
        let gap = 6.0;
        // Fixed footprints: label + value geometry never changes with digit width.
        let widths = [
            ("REALTIME", 0.22_f32),
            ("EMU FPS", 0.18),
            ("AUDIO", 0.16),
            ("QUEUE", 0.16),
            ("FRAME P99", 0.28),
        ];
        let total_frac: f32 = widths.iter().map(|(_, f)| *f).sum();
        let cell_gap_total = gap * (widths.len().saturating_sub(1) as f32);
        let usable = (avail - cell_gap_total).max(1.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (label, frac) in widths.iter() {
                let w = (usable * (frac / total_frac)).max(72.0);
                let (value, level) = match *label {
                    "REALTIME" => (fmt_overview_pct(rt), rt_level(rt)),
                    "EMU FPS" => (fmt_overview_fps(host.emu_fps), Health::Info),
                    "AUDIO" => (audio.label.to_string(), audio.level),
                    "QUEUE" => {
                        if host.audio_offline() {
                            ("  —  ".to_string(), Health::Bad)
                        } else {
                            (
                                fmt_overview_queue_pct(host.audio_queue_pct),
                                queue_level(host.audio_queue_pct),
                            )
                        }
                    }
                    "FRAME P99" => (
                        fmt_overview_ms(host.frame_ms_max),
                        frame_level(host.frame_ms_max),
                    ),
                    _ => (String::new(), Health::Info),
                };
                metric_cell(ui, w, label, &value, level);
            }
        });
    });
}

fn draw_graphs(ui: &mut Ui, frame: &DebugFrame<'_>) {
    let host = frame.host;
    card(ui, "GRAPHS", |ui| {
        graph_caption(ui, "REALTIME", "");
        sparkline(
            ui,
            &frame.history.emu_mhz,
            4.2,
            Color32::from_rgb(120, 220, 140),
        );
        graph_caption(ui, "FRAME", &fmt_overview_ms(host.frame_ms_avg));
        sparkline(
            ui,
            &frame.history.frame_ms,
            20.0,
            Color32::from_rgb(200, 160, 80),
        );
        graph_caption(
            ui,
            "AUDIO Q",
            &fmt_audio_queue(host.audio_queued, host.audio_target),
        );
        sparkline(
            ui,
            &frame.history.audio_queue,
            host.audio_target.max(1) as f32,
            Color32::from_rgb(100, 160, 255),
        );
    });
}

fn graph_caption(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [72.0, 12.0],
            egui::Label::new(
                RichText::new(label)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [120.0, 12.0],
            egui::Label::new(
                RichText::new(value)
                    .font(FontId::monospace(10.0))
                    .color(Color32::DARK_GRAY),
            )
            .halign(egui::Align::LEFT),
        );
    });
}

#[derive(Clone, Copy)]
enum PanelId {
    Performance,
    Audio,
    Profile,
    Cpu,
    Apu,
    Ppu,
    Input,
}

fn visible_panels(frame: &DebugFrame<'_>) -> Vec<PanelId> {
    // Always include; collapse state is handled per-panel.
    let _ = frame;
    vec![
        PanelId::Performance,
        PanelId::Audio,
        PanelId::Profile,
        PanelId::Cpu,
        PanelId::Apu,
        PanelId::Ppu,
        PanelId::Input,
    ]
}

fn draw_panel_grid(ui: &mut Ui, frame: &mut DebugFrame<'_>, panels: &[PanelId], cols: usize) {
    let gap = 8.0;
    let width = ui.available_width();
    let col_w = ((width - gap * (cols as f32 - 1.0).max(0.0)) / cols as f32).max(220.0);

    let mut row: Vec<PanelId> = Vec::new();
    for (i, &panel) in panels.iter().enumerate() {
        row.push(panel);
        let flush = row.len() == cols || i + 1 == panels.len();
        if flush {
            ui.horizontal_top(|ui| {
                for (j, p) in row.iter().enumerate() {
                    if j > 0 {
                        ui.add_space(gap);
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(col_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(col_w - 4.0);
                            draw_panel(ui, frame, *p);
                        },
                    );
                }
            });
            ui.add_space(gap);
            row.clear();
        }
    }
}

fn draw_panel(ui: &mut Ui, frame: &mut DebugFrame<'_>, id: PanelId) {
    match id {
        PanelId::Performance => {
            collapsing(ui, "PERFORMANCE", &mut frame.sections.performance, |ui| {
                let host = frame.host;
                mono_kv(
                    ui,
                    "CYCLES",
                    &format!("{:>5.3}M / 4.194M", host.emu_tcycles_per_sec / 1e6),
                );
                mono_kv(ui, "REALTIME", &fmt_overview_pct(host.realtime_pct()));
                mono_kv(ui, "EMU FPS", &fmt_overview_fps(host.emu_fps));
                mono_kv(ui, "FRAME", &fmt_overview_ms(host.frame_ms_avg));
                mono_kv(
                    ui,
                    "RANGE",
                    &format!(
                        "{:>5.2}–{:>5.2} ms",
                        host.frame_ms_min.clamp(0.0, 999.99),
                        host.frame_ms_max.clamp(0.0, 999.99)
                    ),
                );
                mono_kv(
                    ui,
                    "RENDER",
                    &format!(
                        "{:>5.2}/{:>5.2}",
                        host.render_ms_avg.clamp(0.0, 999.99),
                        host.render_ms_max.clamp(0.0, 999.99)
                    ),
                );
                mono_kv(ui, "SLEEP", &fmt_overview_ms(host.pace_sleep_ms));
                mono_kv(ui, "MISS", &fmt_count_capped(host.missed_frames));
                mono_kv(ui, "REDRAW", &fmt_count_capped(host.redraws));
                mono_kv(
                    ui,
                    "INPUT/S",
                    &format!("{:>6.1}", host.input_events_per_sec.clamp(0.0, 9999.9)),
                );
            })
        }
        PanelId::Audio => collapsing(ui, "AUDIO", &mut frame.sections.audio, |ui| {
            let host = frame.host;
            let fill = (host.audio_queue_pct / 100.0).clamp(0.0, 1.5) as f32 / 1.5;
            const BAR_W: f32 = 220.0;
            bar(ui, fill.min(1.0), Color32::from_rgb(80, 160, 255), BAR_W);
            mono_kv(
                ui,
                "BUFFER",
                &format!(
                    "{}  ({})",
                    fmt_audio_queue(host.audio_queued, host.audio_target),
                    fmt_overview_queue_pct(host.audio_queue_pct).trim()
                ),
            );
            mono_kv(
                ui,
                "EVENTS",
                &format!(
                    "u{} miss{} ov{}",
                    fmt_count_capped(host.audio_underrun_events).trim(),
                    fmt_count_capped(host.audio_missing_samples).trim(),
                    fmt_count_capped(host.audio_overruns).trim()
                ),
            );
            mono_kv(ui, "STEP", &format!("{:>7.5}", host.resample_step));
            mono_kv(
                ui,
                "RATE",
                &if host.audio_offline() {
                    "     — Hz".to_string()
                } else {
                    format!("{:>6} Hz", host.audio_sample_rate.min(999_999))
                },
            );
            mono_kv(
                ui,
                "DEVICE",
                &if host.audio_offline() {
                    host.audio_init_error
                        .as_deref()
                        .unwrap_or("unavailable")
                        .to_string()
                } else if host.audio_device.is_empty() {
                    "—".to_string()
                } else {
                    host.audio_device.clone()
                },
            );
            mono_kv(
                ui,
                "STREAM",
                &if host.audio_offline() {
                    "not initialized".to_string()
                } else {
                    format!(
                        "cb/s {:>6.1}",
                        if host.audio_elapsed_secs > 0.05 {
                            host.audio_callbacks as f64 / host.audio_elapsed_secs
                        } else {
                            0.0
                        }
                    )
                },
            );
            mono_kv(
                ui,
                "P/C",
                &format!(
                    "{}/{}",
                    fmt_count_capped(host.audio_produced).trim(),
                    fmt_count_capped(host.audio_consumed).trim()
                ),
            );
            mono_kv(
                ui,
                "PEAK",
                &format!("{:>5.2} / {:>5.2}", host.peak_l, host.peak_r),
            );
            sparkline(
                ui,
                &frame.history.audio_queue,
                host.audio_target.max(1) as f32,
                Color32::from_rgb(100, 160, 255),
            );
        }),
        PanelId::Profile => collapsing(ui, "PROFILE", &mut frame.sections.profile, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("metric")
                        .font(FontId::monospace(10.0))
                        .color(Color32::GRAY),
                );
                if ui
                    .selectable_label(!*frame.profile_show_max, RichText::new("Avg").monospace())
                    .clicked()
                {
                    *frame.profile_show_max = false;
                }
                if ui
                    .selectable_label(*frame.profile_show_max, RichText::new("Max").monospace())
                    .clicked()
                {
                    *frame.profile_show_max = true;
                }
            });
            ui.label(
                RichText::new("* CPU excludes Bus::tick")
                    .font(FontId::monospace(9.0))
                    .color(Color32::DARK_GRAY),
            );
            let show_max = *frame.profile_show_max;
            // Always paint every profile row — filtering near-zero entries made the
            // list jump as timings flickered across the threshold.
            for (name, stat) in &frame.host.profile_summary.rows {
                let ms = if show_max { stat.max_ms } else { stat.avg_ms };
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [108.0, 12.0],
                        egui::Label::new(
                            RichText::new(*name)
                                .font(FontId::monospace(10.0))
                                .color(Color32::LIGHT_GRAY),
                        ),
                    );
                    ui.label(RichText::new(format!("{ms:5.2}")).font(FontId::monospace(10.0)));
                    ui.label(
                        RichText::new(format!("{:4.1}%", stat.pct))
                            .font(FontId::monospace(10.0))
                            .color(Color32::GRAY),
                    );
                    let bar_w = 56.0;
                    let w = (stat.pct as f32 / 100.0).clamp(0.0, 1.0) * bar_w;
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(bar_w, 7.0), Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(35));
                    let mut fill = rect;
                    fill.set_width(w);
                    ui.painter()
                        .rect_filled(fill, 1.0, Color32::from_rgb(200, 160, 60));
                    resp.on_hover_text(format!(
                        "avg {:.2} ms · max {:.2} ms · {:.1}% of frame",
                        stat.avg_ms, stat.max_ms, stat.pct
                    ));
                });
            }
            ui.separator();
            mono_kv(
                ui,
                "TOTAL",
                &format!(
                    "{:.2} / {:.2} ms",
                    frame.host.profile_summary.frame_avg_ms, frame.host.profile_summary.budget_ms
                ),
            );
        }),
        PanelId::Cpu => collapsing(ui, "CPU", &mut frame.sections.cpu, |ui| {
            if let Some(m) = frame.machine {
                let c = &m.cpu;
                reg_row(ui, "PC", c.pc, "SP", c.sp);
                reg_row(ui, "AF", c.af, "BC", c.bc);
                reg_row(ui, "DE", c.de, "HL", c.hl);
                mono_kv(
                    ui,
                    "FLG",
                    &format!("Z{}N{}H{}C{}", b(c.z), b(c.n), b(c.h), b(c.c)),
                );
                mono_kv(ui, "IME", if c.ime { "ON " } else { "OFF" });
                mono_kv(ui, "OP", &c.mnemonic);
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Apu => collapsing(ui, "APU", &mut frame.sections.apu, |ui| {
            if let Some(m) = frame.machine {
                let a = &m.apu;
                mono_kv(ui, "NR52", &format!("{:02X}", a.nr52));
                channel(ui, "CH1", &a.ch1);
                channel(ui, "CH2", &a.ch2);
                channel(ui, "CH3", &a.ch3);
                channel(ui, "CH4", &a.ch4);
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Ppu => collapsing(ui, "PPU / TIMER", &mut frame.sections.ppu, |ui| {
            if let Some(m) = frame.machine {
                let p = &m.ppu;
                ui.horizontal(|ui| {
                    mono_inline(ui, "LY", &format!("{}", p.ly));
                    mono_inline(ui, "MODE", &format!("{}", p.mode));
                });
                mono_kv(ui, "LCDC", &format!("{:02X}", p.lcdc));
                mono_kv(ui, "STAT", &format!("{:02X}", p.stat));
                mono_kv(ui, "M3", &format!("{}", p.mode3_len));
                mono_kv(ui, "FRAME", &format!("{}", p.frame_index));
                let t = &m.timer;
                mono_kv(
                    ui,
                    "TIM",
                    &format!("DIV={:02X} TIMA={:02X} TAC={:02X}", t.div, t.tima, t.tac),
                );
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Input => collapsing(ui, "INPUT", &mut frame.sections.input, |ui| {
            mono_kv(
                ui,
                "RATE",
                &format!("{:.1} /s", frame.host.input_events_per_sec),
            );
            ui.horizontal(|ui| {
                if let Some(m) = frame.machine {
                    for bttn in GameBoyButton::ALL {
                        let on = m.input.is_pressed(bttn);
                        let label = match bttn {
                            GameBoyButton::Left => "←",
                            GameBoyButton::Up => "↑",
                            GameBoyButton::Right => "→",
                            GameBoyButton::Down => "↓",
                            GameBoyButton::A => "A",
                            GameBoyButton::B => "B",
                            GameBoyButton::Start => "ST",
                            GameBoyButton::Select => "SE",
                        };
                        let c = if on {
                            Color32::from_rgb(80, 220, 120)
                        } else {
                            Color32::from_gray(60)
                        };
                        ui.colored_label(c, RichText::new(label).monospace());
                    }
                    ui.monospace(format!("  P1={:02X}", m.input.p1));
                }
            });
            ui.label(
                RichText::new("F12 toggle · F9 capture")
                    .font(FontId::monospace(9.0))
                    .color(Color32::DARK_GRAY),
            );
        }),
    }
}

fn column_count(width: f32) -> usize {
    if width >= 980.0 {
        3
    } else if width >= 640.0 {
        2
    } else {
        1
    }
}

#[derive(Clone, Copy)]
enum Health {
    Good,
    Warn,
    Bad,
    Info,
}

struct AudioHealth {
    label: &'static str,
    level: Health,
}

fn audio_health(host: &HostMetrics) -> AudioHealth {
    if host.audio_offline() {
        return AudioHealth {
            label: "OFF ",
            level: Health::Bad,
        };
    }
    let underrun_active = host.audio_underrun_events > 0 && host.audio_queue_pct < 50.0;
    if underrun_active {
        AudioHealth {
            // Fixed 4-char status; color carries severity.
            label: "BAD ",
            level: Health::Bad,
        }
    } else {
        match queue_level(host.audio_queue_pct) {
            Health::Good => AudioHealth {
                label: "OK  ",
                level: Health::Good,
            },
            Health::Warn => AudioHealth {
                label: "WARN",
                level: Health::Warn,
            },
            Health::Bad => AudioHealth {
                label: "BAD ",
                level: Health::Bad,
            },
            Health::Info => AudioHealth {
                label: "OK  ",
                level: Health::Info,
            },
        }
    }
}

fn rt_level(pct: f64) -> Health {
    if pct >= 99.0 {
        Health::Good
    } else if pct >= 97.0 {
        Health::Warn
    } else {
        Health::Bad
    }
}

fn queue_level(pct: f64) -> Health {
    if (70.0..=130.0).contains(&pct) {
        Health::Good
    } else if (40.0..70.0).contains(&pct) || (130.0..=160.0).contains(&pct) {
        Health::Warn
    } else {
        Health::Bad
    }
}

fn frame_level(max_ms: f64) -> Health {
    if max_ms <= 18.0 {
        Health::Good
    } else if max_ms <= 28.0 {
        Health::Warn
    } else {
        Health::Bad
    }
}

fn health_color(h: Health) -> Color32 {
    match h {
        Health::Good => Color32::from_rgb(80, 200, 120),
        Health::Warn => Color32::from_rgb(220, 180, 60),
        Health::Bad => Color32::from_rgb(220, 80, 80),
        Health::Info => Color32::from_rgb(100, 160, 220),
    }
}

fn metric_cell(ui: &mut Ui, width: f32, key: &str, val: &str, h: Health) {
    let height = 34.0;
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_min_size(Vec2::new(width, height));
            ui.set_max_width(width);
            ui.add_sized(
                [width, 12.0],
                egui::Label::new(
                    RichText::new(key)
                        .font(FontId::monospace(9.0))
                        .color(Color32::GRAY),
                )
                .truncate(),
            );
            ui.add_sized(
                [width, 16.0],
                egui::Label::new(
                    RichText::new(val)
                        .font(FontId::monospace(13.0))
                        .color(health_color(h))
                        .strong(),
                )
                .halign(egui::Align::RIGHT)
                .truncate(),
            );
        },
    );
}

fn reg_row(ui: &mut Ui, a_name: &str, a_val: u16, b_name: &str, b_val: u16) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_sized(
            [28.0, 14.0],
            egui::Label::new(
                RichText::new(a_name)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [40.0, 14.0],
            egui::Label::new(RichText::new(format!("{a_val:04X}")).font(FontId::monospace(11.0))),
        );
        ui.add_space(16.0);
        ui.add_sized(
            [28.0, 14.0],
            egui::Label::new(
                RichText::new(b_name)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [40.0, 14.0],
            egui::Label::new(RichText::new(format!("{b_val:04X}")).font(FontId::monospace(11.0))),
        );
    });
}

fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(8))
        .corner_radius(2.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .font(FontId::monospace(10.0))
                    .color(Color32::from_rgb(140, 180, 160))
                    .strong(),
            );
            ui.add_space(2.0);
            add(ui);
        });
}

fn collapsing(ui: &mut Ui, title: &str, open: &mut bool, add: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .corner_radius(2.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let resp = CollapsingHeader::new(
                RichText::new(title)
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(140, 180, 160))
                    .strong(),
            )
            .default_open(*open)
            .show(ui, |ui| {
                add(ui);
            });
            *open = resp.openness > 0.5;
        });
}

fn mono_kv(ui: &mut Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [56.0, 12.0],
            egui::Label::new(
                RichText::new(k)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.label(RichText::new(v).font(FontId::monospace(11.0)));
    });
}

fn mono_inline(ui: &mut Ui, k: &str, v: &str) {
    ui.label(
        RichText::new(format!("{k} "))
            .font(FontId::monospace(10.0))
            .color(Color32::GRAY),
    );
    ui.label(RichText::new(v).font(FontId::monospace(11.0)));
    ui.add_space(8.0);
}

fn b(v: bool) -> char {
    if v { '1' } else { '0' }
}

fn channel(ui: &mut Ui, name: &str, ch: &graycart::ChannelDebug) {
    let color = if ch.active {
        Color32::from_rgb(220, 180, 60)
    } else {
        Color32::from_gray(70)
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(name)
                .font(FontId::monospace(10.0))
                .color(if ch.active {
                    Color32::LIGHT_GRAY
                } else {
                    Color32::DARK_GRAY
                }),
        );
        bar(ui, ch.volume as f32 / 15.0, color, 48.0);
        ui.label(
            RichText::new(if ch.active { "ON" } else { "off" })
                .font(FontId::monospace(10.0))
                .color(if ch.active {
                    Color32::from_rgb(80, 200, 120)
                } else {
                    Color32::DARK_GRAY
                }),
        );
        ui.label(
            RichText::new(format!("v{} f{}", ch.volume, ch.frequency))
                .font(FontId::monospace(10.0))
                .color(Color32::GRAY),
        );
    });
}

fn bar(ui: &mut Ui, fill: f32, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(35));
    let mut fill_r = rect;
    fill_r.set_width(rect.width() * fill.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill_r, 1.0, color);
}

fn sparkline(ui: &mut Ui, spark: &Sparkline, hint_max: f32, color: Color32) {
    let samples = spark.samples();
    // Fixed geometry: height is constant; width is the parent's available width.
    const HEIGHT: f32 = 22.0;
    let width = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(20));
    if samples.len() < 2 {
        return;
    }
    let max_v = samples
        .iter()
        .cloned()
        .fold(hint_max.max(0.001), f32::max)
        .max(0.001);
    let n = (samples.len() - 1) as f32;
    let points: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                rect.left() + (i as f32 / n) * rect.width(),
                rect.bottom() - (v / max_v).clamp(0.0, 1.0) * rect.height(),
            )
        })
        .collect();
    for w in points.windows(2) {
        ui.painter()
            .line_segment([w[0], w[1]], Stroke::new(1.1_f32, color));
    }
}

/// Stable overview formats — digit-width changes must not reflow the panel.
fn fmt_overview_pct(v: f64) -> String {
    format!("{:>7.2}%", v.clamp(0.0, 9999.99))
}

fn fmt_overview_fps(v: f64) -> String {
    format!("{:>6.2}", v.clamp(0.0, 9999.99))
}

fn fmt_overview_ms(v: f64) -> String {
    format!("{:>6.2} ms", v.clamp(0.0, 9999.99))
}

fn fmt_overview_queue_pct(v: f64) -> String {
    if v >= 9999.0 {
        "9999+%".to_string()
    } else {
        format!("{:>4.0}%", v.clamp(0.0, 9998.0))
    }
}

fn fmt_audio_queue(queued: usize, target: usize) -> String {
    let q = queued.min(999_999);
    let t = target.min(999_999);
    format!("{q:>6} / {t:<6}")
}

fn fmt_count_capped(n: u64) -> String {
    if n > 999_999 {
        "999999+".to_string()
    } else {
        format!("{n:>7}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        audio_health, fmt_audio_queue, fmt_count_capped, fmt_overview_fps, fmt_overview_ms,
        fmt_overview_pct, fmt_overview_queue_pct,
    };
    use crate::frontend::debug::host::HostMetrics;

    #[test]
    fn overview_formats_have_stable_widths() {
        assert_eq!(fmt_overview_pct(9.9).len(), fmt_overview_pct(100.0).len());
        assert_eq!(
            fmt_overview_pct(99.64).len(),
            fmt_overview_pct(100.00).len()
        );
        assert_eq!(fmt_overview_fps(9.9).len(), fmt_overview_fps(59.73).len());
        assert_eq!(fmt_overview_ms(4.97).len(), fmt_overview_ms(36.96).len());
        assert_eq!(
            fmt_overview_queue_pct(9.0).len(),
            fmt_overview_queue_pct(252.0).len()
        );
        assert_eq!(fmt_count_capped(0).len(), fmt_count_capped(1_424).len());
        assert_eq!(fmt_count_capped(999_999).len(), "999999+".len());
        assert_eq!(
            fmt_audio_queue(9, 2400).len(),
            fmt_audio_queue(2394, 2400).len()
        );
    }

    #[test]
    fn zero_host_rate_is_audio_offline_not_healthy_queue() {
        let host = HostMetrics {
            audio_sample_rate: 0,
            audio_target: 0,
            audio_queue_pct: 0.0,
            audio_underrun_events: 0,
            ..HostMetrics::default()
        };
        let audio = audio_health(&host);
        assert_eq!(audio.label, "OFF ");
        assert!(host.audio_offline());
    }
}

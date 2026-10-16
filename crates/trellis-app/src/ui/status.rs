//! Bottom status bar, the activity log panel, and toast notifications.

use super::common::status_label_ui;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use eframe::egui::{self, Align, Align2, Layout, RichText, Stroke, vec2};
use trellis_core::settings::Corner;

pub fn status_bar(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    egui::TopBottomPanel::bottom("status")
        .exact_height(26.0)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if let Some(fs) = &st.forge {
                    if let Some(h) = &fs.host {
                        ui.label(RichText::new(h.provider.name()).color(color::blue()).strong());
                        ui.label(RichText::new(h.display()).color(color::weak()));
                        let n = fs.prs.len();
                        status_label_ui(ui, &fs.host_status, &format!("{n} open {}{}", fs.pr_noun(), if n == 1 { "" } else { "s" }), msgs);
                        if let Some(a) = fs.auth.get(&h.account().key()) {
                            ui.label(RichText::new(format!("· via {}", a.source)).color(color::faint()));
                        }
                    }
                    if let Some(t) = fs.tracker.as_ref().filter(|t| Some(*t) != fs.host.as_ref()) {
                        ui.label(RichText::new("·").color(color::faint()));
                        ui.label(RichText::new(t.provider.name()).color(color::blue()).strong());
                        status_label_ui(ui, &fs.tracker_status, &format!("{} assigned", fs.my_issues.len()), msgs);
                    }
                    if fs.loading > 0 {
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(concat!("Trellis ", env!("CARGO_PKG_VERSION"))).color(color::faint()));
                    if let Some(job) = &st.job {
                        ui.label(RichText::new(format!("{}…", job.label)).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    } else if st.loading {
                        ui.label(RichText::new("Loading…").color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                });
            });
        });
}

pub fn activity_log(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.view.show_log {
        return;
    }
    egui::TopBottomPanel::bottom("activity").resizable(true).default_height(150.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.strong("Activity");
            if ui.small_button("Clear").clicked() {
                msgs.push(Msg::ClearLog);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Hide").clicked() {
                    st.view.show_log = false;
                }
            });
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for entry in &st.log {
                let c = if entry.ok { color::text() } else { color::red() };
                ui.label(RichText::new(&entry.text).monospace().color(c));
                ui.separator();
            }
        });
    });
}

/// The toasts in the corner from Preferences. The area takes clicks only while a toast has an
/// action button, so plain toasts let clicks through to what is under them.
pub fn toasts(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    st.toasts.retain(|t| t.at.elapsed() < t.lifetime());
    if st.toasts.is_empty() {
        return;
    }
    let (align, offset) = match st.settings.notification_location {
        Corner::BottomLeft => (Align2::LEFT_BOTTOM, vec2(12.0, -36.0)),
        Corner::BottomRight => (Align2::RIGHT_BOTTOM, vec2(-12.0, -36.0)),
        Corner::TopLeft => (Align2::LEFT_TOP, vec2(12.0, 70.0)),
        Corner::TopRight => (Align2::RIGHT_TOP, vec2(-12.0, 70.0)),
    };
    let interactable = st.toasts.iter().any(|t| t.action.is_some());
    egui::Area::new(egui::Id::new("toasts")).anchor(align, offset).interactable(interactable).show(ctx, |ui| {
        for t in &st.toasts {
            // Fade out over the last second.
            let fade = 1.0 - (t.at.elapsed().as_secs_f32() - (t.lifetime().as_secs_f32() - 1.0)).clamp(0.0, 1.0);
            let accent = if t.action.is_some() {
                color::blue()
            } else if t.ok {
                color::green()
            } else {
                color::red()
            };
            egui::Frame::new()
                .fill(color::toolbar().gamma_multiply(fade))
                .stroke(Stroke::new(1.0_f32, accent.gamma_multiply(fade)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(&t.text).color(color::text().gamma_multiply(fade)));
                    if let Some(a) = &t.action {
                        ui.add_space(4.0);
                        ui.scope(|ui| {
                            ui.multiply_opacity(fade);
                            let button = egui::Button::new(&a.label).fill(color::input()).stroke(Stroke::new(1.0_f32, color::border()));
                            if ui.add(button).clicked() {
                                msgs.push(Msg::ToastAction(t.id));
                            }
                        });
                    }
                });
            ui.add_space(6.0);
        }
    });
}

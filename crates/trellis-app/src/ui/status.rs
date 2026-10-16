//! Bottom status bar and toast notifications (the logs panel is in `activity_log.rs`).

use super::common::status_label_ui;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use eframe::egui::{self, Align, Align2, Layout, RichText, Stroke, vec2};
use trellis_core::i18n::Lang;
use trellis_core::settings::Corner;

/// "3 open pull requests" (or merge requests, as the service calls them).
fn open_requests_text(t: Lang, noun: &str, n: usize) -> String {
    if noun == "merge request" { t.trn("status.open_merge_requests", n, &[]) } else { t.trn("status.open_pull_requests", n, &[]) }
}

pub fn status_bar(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    egui::TopBottomPanel::bottom("status")
        .exact_height(26.0)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if let Some(fs) = &st.forge {
                    if let Some(h) = &fs.host {
                        ui.label(RichText::new(h.provider.name()).color(color::blue()).strong());
                        ui.label(RichText::new(h.display()).color(color::weak()));
                        let open = open_requests_text(t, fs.pr_noun(), fs.prs.len());
                        status_label_ui(ui, t, &fs.host_status, &open, msgs);
                        if let Some(a) = fs.auth.get(&h.account().key()) {
                            ui.label(RichText::new(t.trf("status.via", &[("source", &a.source)])).color(color::faint()));
                        }
                    }
                    if let Some(tracker) = fs.tracker.as_ref().filter(|h| Some(*h) != fs.host.as_ref()) {
                        ui.label(RichText::new("·").color(color::faint()));
                        ui.label(RichText::new(tracker.provider.name()).color(color::blue()).strong());
                        let assigned = t.trf("status.assigned", &[("count", &fs.my_issues.len())]);
                        status_label_ui(ui, t, &fs.tracker_status, &assigned, msgs);
                    }
                    if fs.loading > 0 {
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(concat!("Trellis ", env!("CARGO_PKG_VERSION"))).color(color::faint()));
                    super::update_notice::status_item(ui, st, msgs);
                    super::layout::status_items(ui, st, msgs);
                    if let Some(job) = &st.job {
                        ui.label(RichText::new(format!("{}…", job.label)).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    } else if st.loading {
                        ui.label(RichText::new(t.tr("status.loading")).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_requests_text_counts_in_the_language() {
        assert_eq!(open_requests_text(Lang::En, "pull request", 1), "1 open pull request");
        assert_eq!(open_requests_text(Lang::En, "merge request", 2), "2 open merge requests");
        assert_eq!(open_requests_text(Lang::En, "pull request", 0), "0 open pull requests");
        let de = open_requests_text(Lang::De, "pull request", 2);
        assert_eq!(de, Lang::De.trn("status.open_pull_requests", 2, &[]));
        assert!(de.starts_with("2 ") && de != "2 open pull requests", "{de}");
        assert_eq!(open_requests_text(Lang::De, "merge request", 1), Lang::De.trn("status.open_merge_requests", 1, &[]));
    }
}

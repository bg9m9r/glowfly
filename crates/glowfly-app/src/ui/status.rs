//! The bottom status bar: the hosting service and tracker with the counts their sidebar lists
//! show (`state::status_counts`), the job running (or a load, or a fresh build of the search's
//! whole-history index), and the zoom and Show toolbar items (toasts are in `toasts.rs`, the
//! logs panel in `activity_log.rs`). Its parts are joined by "·" only when present.

use super::common::status_label_ui;
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use crate::state::status_counts::{issues_text, prs_text};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText};

pub fn status_bar(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    egui::TopBottomPanel::bottom("status")
        .exact_height(26.0)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if let Some(fs) = &st.session.forge {
                    let filters = st.snap.as_ref().map(|s| st.settings.forge_filters_for(&s.real_workdir)).unwrap_or_default();
                    if let Some(h) = &fs.host {
                        ui.label(RichText::new(h.provider.name()).color(color::text()).font(type_scale::strong(ui, type_scale::BODY)));
                        ui.label(RichText::new(h.display()).color(color::weak()));
                        status_label_ui(ui, t, &fs.host_status, prs_text(t, fs, &filters).as_deref(), msgs);
                        if let Some(a) = fs.auth.get(&h.account().key()) {
                            ui.label(RichText::new(t.trf("status.via", &[("source", &a.source)])).color(color::faint()));
                        }
                    }
                    if let Some(tracker) = fs.tracker.as_ref().filter(|h| Some(*h) != fs.host.as_ref()) {
                        // After the hosting service's part, which always ends in text.
                        if fs.host.is_some() {
                            ui.label(RichText::new("·").color(color::faint()));
                        }
                        ui.label(
                            RichText::new(tracker.provider.name()).color(color::text()).font(type_scale::strong(ui, type_scale::BODY)),
                        );
                        status_label_ui(ui, t, &fs.tracker_status, issues_text(t, fs, &filters).as_deref(), msgs);
                    }
                    if fs.loading > 0 {
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("Glowfly {}", crate::state::menu_bar::VERSION)).color(color::faint()));
                    super::live_updates::status_item(ui, st, msgs);
                    super::update_notice::status_item(ui, st, msgs);
                    super::layout::status_items(ui, st, msgs);
                    if st.job.is_some() {
                        job_item(ui, st, msgs);
                    } else if st.partial.sizing.is_some() {
                        ui.label(RichText::new(t.tr("status.sizing")).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    } else if st.loading {
                        ui.label(RichText::new(t.tr("status.loading")).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    } else if let Some((done, total)) = st.indexing() {
                        let text = match total {
                            Some(total) => t.trf("status.indexing.total", &[("done", &done), ("total", &total)]),
                            None => t.trf("status.indexing", &[("done", &done)]),
                        };
                        ui.label(RichText::new(text).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                });
            });
        });
}

/// The running job (right to left: Cancel when it can be stopped, or disabled with the reason
/// while it is on a step that can't be, its transfer's progress, its label, a spinner).
fn job_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(job) = &st.job else { return };
    let t = st.settings.lang();
    let label = crate::state::job_spec::labels::shown(t, &job.label);
    let named: [(&str, &dyn std::fmt::Display); 1] = [("job", &label)];
    if st.job_cancellable() {
        let step = st.job_stop.as_ref().map(|s| s.step).unwrap_or_default();
        let tip = cancel_tip(t, step, &named);
        let cancel = buttons::quiet(t.tr("common.cancel.button")).height(20.0).tip(&tip);
        if ui.add(cancel).clicked() {
            msgs.push(Msg::Job(JobMsg::Cancel));
        }
    } else if let Some(step) = st.job_cancel_blocked() {
        let why = cancel_tip(t, step, &named);
        ui.add(buttons::quiet(t.tr("common.cancel.button")).height(20.0).enabled(false).why(Some(&why)));
    }
    if let Some(p) = st.job_progress() {
        ui.label(RichText::new(crate::state::job_stop::progress_text(t, p)).color(color::faint()));
    }
    let text = if st.job_cancelling() { t.trf("status.job.cancelling", &named) } else { format!("{label}…") };
    ui.label(RichText::new(text).color(color::weak()));
    ui.add(egui::Spinner::new().size(12.0));
}

/// What Cancel says while the job runs `step`: what a stop does, or why it can't stop now.
fn cancel_tip(t: glowfly_core::i18n::Lang, step: crate::state::job_stop::Step, job: &[(&str, &dyn std::fmt::Display)]) -> String {
    use crate::state::job_stop::Step;
    match step {
        Step::Fetch => t.trf("status.job.cancel.tip", job),
        Step::Push => t.trf("status.job.cancel.push_tip", job),
        Step::Clone => t.trf("status.job.cancel.clone_tip", job),
        Step::Merge => t.trf("status.job.cancel.merging_tip", job),
        Step::Rebase => t.trf("status.job.cancel.rebasing_tip", job),
        Step::Local => t.trf("status.job.cancel.local_tip", job),
    }
}

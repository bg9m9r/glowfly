//! The bottom status bar (design pass D15). Left: the job running as dots, its text and Cancel
//! (or a load, or a fresh build of the search's whole-history index); with none, the active
//! tab's last failure ("Fetch failed · Details", `status_glance.rs`); then the hosting service
//! and tracker with the counts their sidebar lists show (`state::status_counts`). Right: the
//! What's new and update chips and Show toolbar, then when the repository was last fetched
//! (`status_glance.rs`), the live state and the zoom level while it isn't 100%. The version is
//! in About. Toasts are in `toasts.rs`, the logs panel in `activity_log.rs`. Parts are joined by
//! "·" only when present. With Announcements on (Preferences > Accessibility) the job's text is
//! a live region, so screen readers say when a job starts or begins to cancel (its outcome is a
//! toast's); a quiet job (auto-fetch) is never announced.

use super::access::{self, Live};
use super::common::status_label_ui;
use super::theme::color;
use super::theme::sizes::ControlSize;
use super::widgets::{buttons, dots};
use crate::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText};
use glowfly_model::msg::{JobMsg, Msg};
use glowfly_model::state::AppState;
use glowfly_model::state::status_counts::{issues_text, prs_text};

/// The job's text's accessibility key while it is announced.
pub(crate) fn job_key() -> egui::Id {
    egui::Id::new("status-job")
}

/// The size of the bar's controls (Cancel, Details): quiet Compact.
pub(crate) const CONTROL: ControlSize = ControlSize::Compact;
/// The bar's height: the row that hosts a quiet `CONTROL` with its clearance (`sizes::fits`).
pub(crate) const BAR_H: f32 = CONTROL.metrics().h + ControlSize::inset(false);
/// The room either side of the bar's dots.
const DOTS_PAD: f32 = 2.0;

pub fn status_bar(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    debug_assert!(super::theme::sizes::fits(BAR_H, CONTROL, false), "the status bar hosts its Cancel");
    egui::TopBottomPanel::bottom("status")
        .exact_height(BAR_H)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let shown = busy_item(ui, st, msgs) || super::status_glance::failure_item(ui, st, msgs);
                if let Some(fs) = &st.session.forge {
                    if shown && (fs.host.is_some() || fs.tracker.is_some()) {
                        ui.label(RichText::new("·").color(color::faint()));
                    }
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
                    super::layout::zoom_item(ui, st, msgs);
                    super::live_updates::status_item(ui, st, msgs);
                    super::status_glance::fetch_item(ui, st, msgs);
                    super::layout::show_toolbar_item(ui, st, msgs);
                    super::update_notice::status_item(ui, st, msgs);
                    super::whats_new_chip::status_item(ui, st, msgs);
                });
            });
        });
}

/// The bar's first part while something runs: the job, a sizing, a load or an index build, each
/// after its dots. Whether it showed anything.
fn busy_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) -> bool {
    let t = st.settings.lang();
    let text = if st.job.is_some() {
        job_item(ui, st, msgs);
        return true;
    } else if st.partial.sizing.is_some() {
        t.tr("status.sizing").to_owned()
    } else if st.loading {
        t.tr("status.loading").to_owned()
    } else if let Some((done, total)) = st.indexing() {
        match total {
            Some(total) => t.trf("status.indexing.total", &[("done", &done), ("total", &total)]),
            None => t.trf("status.indexing", &[("done", &done)]),
        }
    } else {
        return false;
    };
    dots::show(ui, color::accent(), st.motion_reduced(), DOTS_PAD);
    ui.label(RichText::new(text).color(color::weak()));
    true
}

/// The running job, left to right: its dots, its label, its transfer's progress, and Cancel
/// when it can be stopped (or disabled with the reason while it is on a step that can't be).
fn job_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(job) = &st.job else { return };
    let t = st.settings.lang();
    let label = glowfly_model::state::job_spec::labels::shown(t, &job.label);
    let named: [(&str, &dyn std::fmt::Display); 1] = [("job", &label)];
    dots::show(ui, color::accent(), st.motion_reduced(), DOTS_PAD);
    let text = if st.job_cancelling() { t.trf("status.job.cancelling", &named) } else { format!("{label}…") };
    let line = ui.label(RichText::new(text).color(color::weak()));
    // A quiet job (auto-fetch) runs on a timer: announcing it would interrupt every interval.
    if st.settings.a11y.announce_jobs && !job.quiet {
        access::announce(&line, job_key(), Live::Polite);
    }
    if let Some(p) = st.job_progress() {
        ui.label(RichText::new(glowfly_model::state::job_stop::progress_text(t, p)).color(color::faint()));
    }
    if st.job_cancellable() {
        let step = st.job_stop.as_ref().map(|s| s.step).unwrap_or_default();
        let tip = cancel_tip(t, step, &named);
        let cancel = buttons::quiet(t.tr("common.cancel.button")).size(CONTROL).tip(&tip);
        if ui.add(cancel).clicked() {
            msgs.push(Msg::Job(JobMsg::Cancel));
        }
    } else if let Some(step) = st.job_cancel_blocked() {
        let why = cancel_tip(t, step, &named);
        ui.add(buttons::quiet(t.tr("common.cancel.button")).size(CONTROL).enabled(false).why(Some(&why)));
    }
}

/// What Cancel says while the job runs `step`: what a stop does, or why it can't stop now.
fn cancel_tip(t: glowfly_core::i18n::Lang, step: glowfly_model::state::job_stop::Step, job: &[(&str, &dyn std::fmt::Display)]) -> String {
    use glowfly_model::state::job_stop::Step;
    match step {
        Step::Fetch => t.trf("status.job.cancel.tip", job),
        Step::Push => t.trf("status.job.cancel.push_tip", job),
        Step::Clone => t.trf("status.job.cancel.clone_tip", job),
        Step::Merge => t.trf("status.job.cancel.merging_tip", job),
        Step::Rebase => t.trf("status.job.cancel.rebasing_tip", job),
        Step::Local => t.trf("status.job.cancel.local_tip", job),
    }
}

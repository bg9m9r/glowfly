//! The bottom status bar: the hosting service and tracker, the job running, and the zoom and
//! Show toolbar items (toasts are in `toasts.rs`, the logs panel in `activity_log.rs`).

use super::common::status_label_ui;
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText};
use trellis_core::i18n::Lang;

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
                        ui.label(RichText::new(h.provider.name()).color(color::text()).font(type_scale::strong(ui, type_scale::BODY)));
                        ui.label(RichText::new(h.display()).color(color::weak()));
                        let open = open_requests_text(t, fs.pr_noun(), fs.prs.len());
                        status_label_ui(ui, t, &fs.host_status, &open, msgs);
                        if let Some(a) = fs.auth.get(&h.account().key()) {
                            ui.label(RichText::new(t.trf("status.via", &[("source", &a.source)])).color(color::faint()));
                        }
                    }
                    if let Some(tracker) = fs.tracker.as_ref().filter(|h| Some(*h) != fs.host.as_ref()) {
                        ui.label(RichText::new("·").color(color::faint()));
                        ui.label(
                            RichText::new(tracker.provider.name()).color(color::text()).font(type_scale::strong(ui, type_scale::BODY)),
                        );
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
                    if st.job.is_some() {
                        job_item(ui, st, msgs);
                    } else if st.partial.sizing.is_some() {
                        ui.label(RichText::new(t.tr("status.sizing")).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    } else if st.loading {
                        ui.label(RichText::new(t.tr("status.loading")).color(color::weak()));
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                });
            });
        });
}

/// The running job (right to left: Cancel when it can be stopped, its label, a spinner).
fn job_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(job) = &st.job else { return };
    let t = st.settings.lang();
    if st.job_cancellable() {
        let tip = t.trf("status.job.cancel.tip", &[("job", &job.label)]);
        let cancel = buttons::quiet(t.tr("common.cancel.button")).height(20.0).tip(&tip);
        if ui.add(cancel).clicked() {
            msgs.push(Msg::Job(JobMsg::Cancel));
        }
    }
    let text = if st.job_cancelling() { t.trf("status.job.cancelling", &[("job", &job.label)]) } else { format!("{}…", job.label) };
    ui.label(RichText::new(text).color(color::weak()));
    ui.add(egui::Spinner::new().size(12.0));
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

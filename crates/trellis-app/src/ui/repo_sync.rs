//! The fetch or pull across repositories dialog (`Modal::SyncRepos`): how far the run is, then
//! a row per repository with how it went. The run goes on when the dialog is closed; its end
//! is then told in a toast (update/repo_sync.rs).

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::truncate;
use crate::state::repo_sync::{RepoSync, RowState, SyncKind, SyncRow, outcome_text};
use eframe::egui::{self, Align, Layout, RichText};
use trellis_core::native_path::shown;
use trellis_core::repo_sync::Outcome;

/// Why some repositories may fail where an open one would ask.
const NO_PROMPTS_NOTE: &str = "Git doesn't ask for passwords here: a repository whose remote needs one that isn't saved fails.";

/// Draws the dialog (wide, with a resizable list); returns true when it should close.
pub fn show(ui: &mut egui::Ui, cx: Cx, sync: Option<&RepoSync>) -> bool {
    let t = cx.t;
    // Nothing to show without a run.
    let Some(sync) = sync else { return true };
    let mut d = Dialog::new(cx, sync.title()).wide().resizable("repo-sync").begin(ui);
    let (done, all) = sync.progress();
    if sync.running() {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            let verb = match sync.kind {
                SyncKind::Fetch => "Fetching",
                SyncKind::Pull => "Pulling",
            };
            ui.label(format!("{verb}: {done} of {all} done"));
        });
    } else {
        ui.label(sync.summary());
    }
    ui.label(RichText::new(NO_PROMPTS_NOTE).small().color(color::weak()));
    ui.add_space(8.0);
    d.body(ui, |ui| {
        for r in &sync.rows {
            row(ui, sync.kind, r);
        }
    });
    let label = if sync.running() { t.tr("repo_sync.close_running.button") } else { t.tr("common.close.button") };
    d.finish(ui, Footer::new().close_label(label)).dismiss
}

/// The widest a repository's name is drawn before it is cut.
const NAME_WIDTH: f32 = 180.0;

/// One repository: its name (its path in the tip) and where it stands.
fn row(ui: &mut egui::Ui, kind: SyncKind, r: &SyncRow) {
    ui.horizontal(|ui| {
        let size = egui::vec2(NAME_WIDTH, ui.spacing().interact_size.y);
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(NAME_WIDTH);
            // One tooltip: the whole name above the path when the name is cut.
            let (resp, cut) = truncate::cut_label(ui, RichText::new(&r.name).strong(), NAME_WIDTH);
            resp.on_hover_text(if cut { format!("{}\n{}", r.name, shown(&r.path)) } else { shown(&r.path) });
        });
        ui.add_space(12.0);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| match &r.state {
            RowState::Waiting => {
                ui.label(RichText::new("Waiting").color(color::weak()));
            }
            RowState::Running => {
                ui.add(egui::Spinner::new().size(12.0));
                ui.label(RichText::new("Running…").color(color::weak()));
            }
            RowState::Done(o) => {
                let (text, bad) = outcome_text(kind, o);
                let col = match o {
                    Outcome::Untrusted => color::amber(),
                    _ if bad => color::red(),
                    _ => color::text(),
                };
                truncate::truncate_label(ui, RichText::new(&text).color(col), ui.available_width());
            }
        });
    });
}

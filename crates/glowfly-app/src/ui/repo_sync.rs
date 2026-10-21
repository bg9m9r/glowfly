//! The fetch or pull across repositories dialog (`Modal::SyncRepos`): how far the run is, then
//! a row per repository with how it went, Retry on a failed or stopped row and Open on a
//! finished one. Stop keeps waiting repositories from starting. The run goes on when the
//! dialog is closed; its end is then told in a toast (update/repo_sync.rs).

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::{buttons, truncate};
use crate::msg::{Msg, RepoMsg, ReposMsg};
use crate::state::repo_sync::{RepoSync, RowState, SyncKind, SyncRow, can_open, can_retry, failure_tip, outcome_text};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::repo_sync::Outcome;

/// Why some repositories may fail where an open one would ask.
const NO_PROMPTS_NOTE: &str = "Git doesn't ask for passwords here: a repository whose remote needs one that isn't saved fails.";

/// Draws the dialog (wide, with a resizable list); returns true when it should close.
pub fn show(ui: &mut egui::Ui, cx: Cx, sync: Option<&RepoSync>, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    // Nothing to show without a run.
    let Some(sync) = sync else { return true };
    let mut d = Dialog::new(cx, sync.title()).size(Size::Wide).resizable("repo-sync").begin(ui);
    let (done, all) = sync.progress();
    if sync.running() {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            let verb = match sync.kind {
                SyncKind::Fetch => "Fetching",
                SyncKind::Pull => "Pulling",
            };
            match sync.stopping() {
                Some(n) => ui.label(t.trn("repo_sync.stopping", n, &[])),
                None => ui.label(format!("{verb}: {done} of {all} done")),
            };
        });
    } else {
        ui.label(sync.summary());
    }
    ui.label(RichText::new(NO_PROMPTS_NOTE).size(type_scale::SMALL).color(color::weak()));
    ui.add_space(8.0);
    let mut opened = false;
    d.body(ui, |ui| {
        for (index, r) in sync.rows.iter().enumerate() {
            opened |= row(ui, t, sync, index, r, msgs);
        }
    });
    let label = if sync.running() { t.tr("repo_sync.close_running.button") } else { t.tr("common.close.button") };
    let mut footer = Footer::new().close_label(label);
    if sync.running() {
        let stop = buttons::secondary(t.tr("repo_sync.stop.button"))
            .enabled(sync.can_stop())
            .tip(t.tr("repo_sync.stop.tip"))
            .why(Some(t.tr("repo_sync.stop.stopping_tip")));
        footer = footer.extra(stop);
    }
    let out = d.finish(ui, footer);
    if out.extra == Some(0) {
        msgs.push(Msg::Repos(ReposMsg::StopSync));
    }
    out.dismiss || opened
}

/// The widest a repository's name is drawn before it is cut.
const NAME_WIDTH: f32 = 180.0;
/// The column at the right of each row for its Retry and Open buttons, kept on every row so the
/// outcomes line up.
const ACTIONS_W: f32 = 150.0;
/// A row's height: a small button's.
const ROW_H: f32 = 24.0;

/// One repository: its name (its path in the tip), where it stands, and its buttons. Returns
/// true when Open was clicked (the dialog closes).
fn row(ui: &mut egui::Ui, t: Lang, sync: &RepoSync, index: usize, r: &SyncRow, msgs: &mut Vec<Msg>) -> bool {
    let mut opened = false;
    ui.horizontal(|ui| {
        // As tall as the small buttons, so rows with and without them line up.
        let h = ROW_H;
        ui.set_min_height(h);
        ui.allocate_ui_with_layout(egui::vec2(NAME_WIDTH, h), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(NAME_WIDTH);
            // One tooltip: the whole name above the path when the name is cut.
            let (resp, cut) = truncate::cut_label(ui, RichText::new(&r.name).font(type_scale::strong(ui, type_scale::BODY)), NAME_WIDTH);
            resp.on_hover_text(if cut { format!("{}\n{}", r.name, shown(&r.path)) } else { shown(&r.path) });
        });
        ui.add_space(12.0);
        let text_w = (ui.available_width() - ACTIONS_W).max(0.0);
        ui.allocate_ui_with_layout(egui::vec2(text_w, h), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(text_w);
            state(ui, t, sync.kind, &r.state, text_w);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if can_open(r) {
                let open = buttons::secondary(t.tr("repo_sync.open.button")).small();
                if ui.add(open).on_hover_text(t.tr("repo_sync.open.tip")).clicked() {
                    msgs.push(Msg::Repo(RepoMsg::Open(r.path.clone())));
                    opened = true;
                }
            }
            if can_retry(r) && ui.add(buttons::secondary(t.tr("repo_sync.retry.button")).small()).clicked() {
                msgs.push(Msg::Repos(ReposMsg::RetrySync { generation: sync.generation, index }));
            }
        });
    });
    opened
}

/// Where a row stands, in at most `width`: a failure shows git's whole output on hover.
fn state(ui: &mut egui::Ui, t: Lang, kind: SyncKind, s: &RowState, width: f32) {
    match s {
        RowState::Waiting => {
            ui.label(RichText::new("Waiting").color(color::weak()));
        }
        RowState::Running => {
            ui.add(egui::Spinner::new().size(12.0));
            ui.label(RichText::new("Running…").color(color::weak()));
        }
        RowState::Stopped => {
            ui.label(RichText::new(t.tr("repo_sync.row.stopped")).color(color::weak()));
        }
        RowState::Done(o) => {
            let (text, bad) = outcome_text(kind, o);
            let col = match o {
                Outcome::Untrusted => color::warn(),
                _ if bad => color::removed(),
                _ => color::text(),
            };
            match failure_tip(o) {
                Some(tip) => {
                    truncate::cut_label(ui, RichText::new(&text).color(col), width).0.on_hover_text(tip);
                }
                None => {
                    truncate::truncate_label(ui, RichText::new(&text).color(col), width);
                }
            }
        }
    }
}

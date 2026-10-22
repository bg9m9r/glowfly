//! The card over the WIP panel while an operation is in progress (a merge, rebase, cherry-pick,
//! revert, patch series or bisect) or conflicts are left. Top to bottom: the title (with the
//! operation's own lines), one status line ("2 conflicted files left" or "All conflicts
//! resolved"), any notes, then every action in one wrapping row with the standard gap.
//!
//! The row leads with the one next step, in the primary style: View conflicts while files are
//! conflicted, else Continue (Commit merge for a merge). Continue and Commit merge stay out of
//! the row until the conflicts are resolved, as git refuses them before. Skip and Edit
//! remaining steps follow, and Abort or Reset bisect comes last; all but Continue ask first
//! (`state::op_confirm`). A Commit merge blocked for another reason (a job running, an amend)
//! says why on a line under the row. A merge that is part of a stopped Git Flow Finish says so
//! under the title, with the steps left.

use super::common::state_actions;
use super::theme::color;
use super::widgets::banner::{self, BannerKind};
use super::widgets::{buttons, reason};
use crate::msg::{CommitMsg, ConflictMsg, Msg};
use crate::state::AppState;
use crate::state::commit::commit_merge_blocked;
use crate::state::op_actions::{action_text, banner_title};
use crate::state::op_confirm::{Risky, banner_msg};
use crate::state::rebase_banner::action_msg;
use eframe::egui::{self, Align, RichText};
use git2::RepositoryState;
use glowfly_core::git_version::Feature;
use glowfly_core::pick_ops::{CHERRY_PICK_SKIP, REVERT_SKIP};
use glowfly_core::repo::{FileEntry, Snapshot};

/// The gap between the card's buttons, across and between wrapped rows.
const BUTTON_GAP: egui::Vec2 = egui::vec2(8.0, 8.0);

/// The in-progress card, with `conflicted` the conflicted files and `resolved` how many were
/// resolved; nothing when no operation is in progress and nothing is conflicted.
pub(super) fn op_banner(
    ui: &mut egui::Ui,
    st: &AppState,
    snap: &Snapshot,
    conflicted: &[&FileEntry],
    resolved: usize,
    msgs: &mut Vec<Msg>,
) {
    let in_progress = state_actions(snap.state);
    if in_progress.is_none() && conflicted.is_empty() {
        return;
    }
    let t = st.settings.lang();
    let conflicts_left = !conflicted.is_empty();
    let kind = if conflicts_left {
        BannerKind::NeedsYou
    } else if st.job.is_some() {
        BannerKind::Progress
    } else {
        BannerKind::Info
    };
    banner::frame(ui, kind, None, |ui| {
        match (banner_title(t, snap), &snap.rebase) {
            (Some(title), Some(p)) => super::rebase::banner::progress(ui, t, &title, p, conflicts_left, msgs),
            (Some(title), None) => super::am_banner::title(ui, t, snap, &title, conflicts_left),
            (None, _) => {}
        }
        if let Some(status) = status_line(st, snap, conflicted.len(), resolved) {
            ui.add(egui::Label::new(RichText::new(status).color(color::text())).wrap());
        }
        if let Some(note) = crate::state::auto_stash::banner_note(st) {
            ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
        }
        if let Some(r) = st.gitflow_resume.as_ref().filter(|r| r.resume.merging(snap)) {
            let text = crate::state::gitflow_resume::waiting_text(t, &r.resume);
            ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
        }
        ui.add_space(4.0);
        let why = ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = BUTTON_GAP;
            actions(ui, st, snap, conflicted, msgs)
        });
        if let Some(why) = why.inner {
            reason::line(ui, Some(&why), Align::Min);
        }
    });
    ui.add_space(8.0);
}

/// "2 conflicted files left" while any are; "All conflicts resolved" once some were, for the
/// operations whose own lines don't already say so (a rebase's stop line, a patch's hint do).
fn status_line(st: &AppState, snap: &Snapshot, conflicted: usize, resolved: usize) -> Option<String> {
    let t = st.settings.lang();
    if conflicted > 0 {
        Some(t.trn("details.banner.conflicted", conflicted, &[]))
    } else if resolved > 0 && snap.rebase.is_none() && snap.am.is_none() {
        Some(t.tr("details.banner.all_resolved").to_owned())
    } else {
        None
    }
}

/// The card's buttons, the next step first and the step that loses work last; returns why
/// Commit merge is disabled, when it is.
fn actions(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, conflicted: &[&FileEntry], msgs: &mut Vec<Msg>) -> Option<String> {
    let t = st.settings.lang();
    let mut blocked = None;
    if let Some(first) = conflicted.first() {
        if ui.add(buttons::primary(t.tr("details.banner.view_conflicts.button"))).clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::OpenConflict(first.path.clone())));
        }
    } else if snap.state == RepositoryState::Merge {
        let why = commit_merge_blocked(st);
        let b = buttons::primary(action_text(t, "Commit merge")).enabled(why.is_none()).why(why.as_deref());
        if ui.add(b).clicked() {
            msgs.push(Msg::Commit(CommitMsg::CommitMerge));
        }
        blocked = why;
    }
    let Some(steps) = state_actions(snap.state) else { return blocked };
    let skip_blocked = st.git.blocked(Feature::SequencerSkip);
    for (label, a) in steps.iter().filter(|(_, a)| !destructive(a)) {
        let cont = *label == "Continue";
        if cont && !conflicted.is_empty() {
            continue;
        }
        let old_git = (*a == CHERRY_PICK_SKIP || *a == REVERT_SKIP).then_some(skip_blocked.as_deref()).flatten();
        let text = action_text(t, label);
        let button = if cont { buttons::primary(text) } else { buttons::secondary(text) };
        if ui.add(button.enabled(old_git.is_none()).why(old_git)).clicked() {
            msgs.push(if cont { action_msg(snap.state, label, a) } else { banner_msg(st, label, a) });
        }
    }
    super::rebase::banner::edit_steps_button(ui, t, snap.state, msgs);
    for (label, a) in steps.iter().filter(|(_, a)| destructive(a)) {
        if ui.add(buttons::secondary(action_text(t, label))).clicked() {
            msgs.push(banner_msg(st, label, a));
        }
    }
    blocked
}

/// Abort and Reset bisect: last in the row (the confirmation they open is the danger one).
fn destructive(args: &[&str]) -> bool {
    matches!(Risky::of(args), Some(Risky::Abort | Risky::ResetBisect))
}

//! The banner over the WIP panel while an operation is in progress (a merge, rebase,
//! cherry-pick, revert, patch series or bisect) or conflicts are left: its title, the conflicted
//! files with View conflicts, and the actions: the primary Continue (Commit merge for a merge)
//! and Skip at the left, Abort or Reset bisect alone at the right; all but Continue ask first
//! (`state::op_confirm`).

use super::common::state_actions;
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{CommitMsg, ConflictMsg, Msg};
use crate::state::AppState;
use crate::state::commit::commit_merge_blocked;
use crate::state::op_actions::action_text;
use crate::state::op_confirm::{Risky, banner_msg};
use crate::state::rebase_banner::action_msg;
use eframe::egui::{self, Align, Layout};
use git2::RepositoryState;
use trellis_core::git_version::Feature;
use trellis_core::pick_ops::{CHERRY_PICK_SKIP, REVERT_SKIP};
use trellis_core::repo::{FileEntry, Snapshot};

/// The in-progress frame, with `conflicted` the conflicted files; nothing when neither applies.
/// Continue waits for the conflicts of a rebase or patch series to be resolved, Commit merge for
/// a merge's, and Skip commit of a cherry-pick or revert needs git 2.23.
pub(super) fn op_banner(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, conflicted: &[&FileEntry], msgs: &mut Vec<Msg>) {
    let in_progress = state_actions(snap.state);
    if in_progress.is_none() && conflicted.is_empty() {
        return;
    }
    let skip_blocked = st.git.blocked(Feature::SequencerSkip);
    let t = st.settings.lang();
    egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
        match (in_progress, &snap.rebase) {
            (Some((name, _)), Some(p)) => super::rebase::banner::progress(ui, p, name, !conflicted.is_empty(), msgs),
            (Some((name, _)), None) => super::am_banner::title(ui, snap, name, !conflicted.is_empty()),
            _ => {}
        }
        if let Some(first) = conflicted.first() {
            ui.horizontal(|ui| {
                ui.label(t.trn("details.banner.conflicted", conflicted.len(), &[]));
                if ui.button(t.tr("details.banner.view_conflicts.button")).clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::OpenConflict(first.path.clone())));
                }
            });
        }
        let Some((_, actions)) = in_progress else { return };
        ui.horizontal(|ui| {
            // The step that loses work alone at the right edge, apart from the others.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for (label, a) in actions.iter().filter(|(_, a)| destructive(a)) {
                    if ui.add(buttons::danger_outline(action_text(t, label))).clicked() {
                        msgs.push(banner_msg(st, label, a));
                    }
                }
                ui.with_layout(Layout::left_to_right(Align::Center).with_main_wrap(true), |ui| {
                    if snap.state == RepositoryState::Merge {
                        commit_merge_button(ui, st, msgs);
                    }
                    for (label, a) in actions.iter().filter(|(_, a)| !destructive(a)) {
                        let cont = *label == "Continue";
                        let held = !conflicted.is_empty() && cont && (snap.rebase.is_some() || snap.am.is_some());
                        let old_git = (*a == CHERRY_PICK_SKIP || *a == REVERT_SKIP).then_some(skip_blocked.as_deref()).flatten();
                        let why = if held { Some(t.tr("details.banner.resolve_first.tip")) } else { old_git };
                        let text = action_text(t, label);
                        let button = if cont { buttons::primary(text, why.is_none()) } else { egui::Button::new(text) };
                        let r = ui.add_enabled(why.is_none(), button);
                        let r = match why {
                            Some(w) => r.on_disabled_hover_text(w),
                            None => r,
                        };
                        if r.clicked() {
                            msgs.push(if cont { action_msg(snap.state, label, a) } else { banner_msg(st, label, a) });
                        }
                    }
                    super::rebase::banner::edit_steps_button(ui, snap.state, msgs);
                });
            });
        });
    });
    ui.add_space(6.0);
}

/// Abort and Reset bisect: drawn apart, in the danger outline.
fn destructive(args: &[&str]) -> bool {
    matches!(Risky::of(args), Some(Risky::Abort | Risky::ResetBisect))
}

/// The merge banner's primary action: commits the merge, once nothing is conflicted.
fn commit_merge_button(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let why = commit_merge_blocked(st);
    let t = st.settings.lang();
    let r = ui.add_enabled(why.is_none(), buttons::primary(action_text(t, "Commit merge"), why.is_none()));
    if r.on_disabled_hover_text(why.unwrap_or_default()).clicked() {
        msgs.push(Msg::Commit(CommitMsg::CommitMerge));
    }
}

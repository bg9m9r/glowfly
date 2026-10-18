//! The banner over the WIP panel while an operation is in progress (a merge, rebase,
//! cherry-pick, revert, patch series or bisect) or conflicts are left: its title, the conflicted
//! files with View conflicts, and Continue / Skip / Abort.

use super::common::state_actions;
use super::theme::color;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::rebase_banner::action_msg;
use eframe::egui;
use trellis_core::git_version::Feature;
use trellis_core::i18n::Lang;
use trellis_core::pick_ops::{CHERRY_PICK_SKIP, REVERT_SKIP};
use trellis_core::repo::{FileEntry, Snapshot};

/// The in-progress frame, with `conflicted` the conflicted files; nothing when neither applies.
/// Continue waits for the conflicts of a rebase or patch series to be resolved, and Skip commit
/// of a cherry-pick or revert needs git 2.23.
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
        ui.horizontal_wrapped(|ui| {
            for (label, a) in actions {
                let held = !conflicted.is_empty() && *label == "Continue" && (snap.rebase.is_some() || snap.am.is_some());
                let old_git = (*a == CHERRY_PICK_SKIP || *a == REVERT_SKIP).then_some(skip_blocked.as_deref()).flatten();
                let why = if held { Some(t.tr("details.banner.resolve_first.tip")) } else { old_git };
                let r = ui.add_enabled(why.is_none(), egui::Button::new(action_text(t, label)));
                let r = match why {
                    Some(w) => r.on_disabled_hover_text(w),
                    None => r,
                };
                if r.clicked() {
                    msgs.push(action_msg(snap.state, label, a));
                }
            }
            super::rebase::banner::edit_steps_button(ui, snap.state, msgs);
        });
    });
    ui.add_space(6.0);
}

/// A banner button's text in the language; `label` stays the English id the job is named by.
fn action_text(t: Lang, label: &'static str) -> &'static str {
    match label {
        "Continue" => t.tr("details.banner.action.continue"),
        "Skip commit" => t.tr("details.banner.action.skip_commit"),
        "Skip patch" => t.tr("details.banner.action.skip_patch"),
        "Abort" => t.tr("details.banner.action.abort"),
        "Abort merge" => t.tr("details.banner.action.abort_merge"),
        "Abort rebase" => t.tr("details.banner.action.abort_rebase"),
        "Abort cherry-pick" => t.tr("details.banner.action.abort_cherry_pick"),
        "Abort revert" => t.tr("details.banner.action.abort_revert"),
        "Reset bisect" => t.tr("details.banner.action.reset_bisect"),
        _ => label,
    }
}

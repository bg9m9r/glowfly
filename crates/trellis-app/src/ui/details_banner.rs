//! The banner over the WIP panel while an operation is in progress (a merge, rebase,
//! cherry-pick, revert, patch series or bisect) or conflicts are left: its title, the conflicted
//! files with View conflicts, and Continue / Skip / Abort.

use super::common::state_actions;
use super::theme::color;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::rebase_banner::{RESOLVE_FIRST, action_msg};
use eframe::egui;
use trellis_core::git_version::Feature;
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
    egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
        match (in_progress, &snap.rebase) {
            (Some((name, _)), Some(p)) => super::rebase::banner::progress(ui, p, name, !conflicted.is_empty(), msgs),
            (Some((name, _)), None) => super::am_banner::title(ui, snap, name, !conflicted.is_empty()),
            _ => {}
        }
        if let Some(first) = conflicted.first() {
            ui.horizontal(|ui| {
                let n = conflicted.len();
                ui.label(format!("{n} conflicted file{}", if n == 1 { "" } else { "s" }));
                if ui.button("View conflicts").clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::OpenConflict(first.path.clone())));
                }
            });
        }
        let Some((_, actions)) = in_progress else { return };
        ui.horizontal_wrapped(|ui| {
            for (label, a) in actions {
                let held = !conflicted.is_empty() && *label == "Continue" && (snap.rebase.is_some() || snap.am.is_some());
                let old_git = (*a == CHERRY_PICK_SKIP || *a == REVERT_SKIP).then_some(skip_blocked.as_deref()).flatten();
                let why = if held { Some(RESOLVE_FIRST) } else { old_git };
                let r = ui.add_enabled(why.is_none(), egui::Button::new(*label));
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

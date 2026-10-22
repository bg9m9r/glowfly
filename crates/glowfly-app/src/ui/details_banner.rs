//! The heading over the WIP panel for an operation that has no steps (`state::op_steps`): a
//! bisect, or git's ambiguous "apply mailbox or rebase" leftover, with nothing conflicted.
//! Neither has conflicts to resolve or a commit to make, so it keeps to the steps' heading: the
//! title with its quiet Reset bisect (or Abort, asking first) at the right, then its notes.

use super::theme::{color, type_scale};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::op_actions::banner_title;
use eframe::egui::{self, RichText};
use glowfly_core::repo::Snapshot;

/// The heading; nothing when no operation is in progress.
pub(super) fn op_banner(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let Some(title) = banner_title(st.settings.lang(), snap) else { return };
    super::op_steps::title_row(ui, st, snap, &title, msgs);
    if let Some(note) = crate::state::auto_stash::banner_note(st) {
        ui.add(egui::Label::new(RichText::new(note).size(type_scale::SMALL).color(color::weak())).wrap());
    }
    ui.add_space(12.0);
}

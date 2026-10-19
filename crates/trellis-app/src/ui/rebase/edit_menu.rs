//! The graph menus' history edits: reword, squash into the parent and drop one commit, or
//! squash and drop a multi-selection. Each opens a dialog or a confirmation, so each label ends
//! in an ellipsis. An entry that can't be used is disabled with a short reason after its label
//! and the whole one on hover (`state::rebase_quick`).

use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase_quick;
use crate::state::selection::MultiSelection;
use crate::ui::widgets::reason::{self, ItemState};
use eframe::egui;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::rebase::QuickEdit;
use trellis_core::repo::Snapshot;

/// One entry: pushes `edit` and closes the menu when clicked.
fn entry(ui: &mut egui::Ui, text: String, blocked: Option<&str>, edit: QuickEdit, msgs: &mut Vec<Msg>) {
    if reason::menu_button(ui, text, "", ItemState::blocked_by(blocked)).clicked() {
        msgs.push(Msg::Rebase(RebaseMsg::Edit(edit)));
        ui.close_menu();
    }
}

/// Reword, squash into parent and drop for the commit `oid`.
pub fn single(ui: &mut egui::Ui, t: Lang, oid: Oid, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let b = rebase_quick::single(snap, oid);
    entry(ui, t.tr("commits.menu.reword").into(), b.reword.as_deref(), QuickEdit::Reword(oid), msgs);
    entry(ui, t.tr("commits.menu.squash_parent").into(), b.squash.as_deref(), QuickEdit::SquashIntoParent(oid), msgs);
    entry(ui, t.tr("commits.menu.drop").into(), b.drop.as_deref(), QuickEdit::Drop(vec![oid]), msgs);
}

/// Squash and drop for the selected commits, in `t`'s words (the menu's header gives the count).
pub fn multi(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let b = rebase_quick::multi(snap, &m.oids);
    entry(ui, t.tr("commits.menu.squash").into(), b.squash.as_deref(), QuickEdit::Squash(m.oids.clone()), msgs);
    entry(ui, t.tr("commits.menu.drop").into(), b.drop.as_deref(), QuickEdit::Drop(m.oids.clone()), msgs);
}

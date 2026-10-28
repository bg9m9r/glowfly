//! The graph menus' history edits: reword, squash into the parent and drop one commit, or
//! squash and drop a multi-selection. Each opens a dialog or a confirmation, so each label ends
//! in an ellipsis. Drop is destructive: its menu puts it last, in the danger style (`drop`). An entry that can't be used is greyed with an info mark, its reason on hover
//! or focus (`state::rebase_quick`).

use crate::msg::{Msg, OpPreviewMsg, RebaseMsg};
use crate::state::rebase_quick;
use crate::state::selection::MultiSelection;
use crate::ui::widgets::reason::{self, ItemState};
use eframe::egui;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::rebase::QuickEdit;
use glowfly_core::repo::Snapshot;

/// One entry: pushes `edit` and closes the menu when clicked.
fn entry(ui: &mut egui::Ui, text: String, blocked: Option<&str>, edit: QuickEdit, msgs: &mut Vec<Msg>) {
    entry_in(ui, text, ItemState::blocked_by(blocked), edit, msgs);
}

fn entry_in(ui: &mut egui::Ui, text: String, state: ItemState<'_>, edit: QuickEdit, msgs: &mut Vec<Msg>) {
    if reason::menu_button(ui, text, "", state).clicked() {
        msgs.push(Msg::Rebase(RebaseMsg::Edit(edit)));
        ui.close_menu();
    }
}

/// "Preview drop…": opens the preview of dropping `oids`, blocked as Drop is.
fn preview(ui: &mut egui::Ui, t: Lang, blocked: Option<&str>, oids: Vec<Oid>, msgs: &mut Vec<Msg>) {
    if reason::menu_button(ui, t.tr("op_preview.menu.drop"), "", ItemState::blocked_by(blocked)).clicked() {
        msgs.push(Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Drop(oids))));
        ui.close_menu();
    }
}

/// Reword and squash into parent for the commit `oid`, and the preview of dropping it.
pub fn single(ui: &mut egui::Ui, t: Lang, oid: Oid, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let b = rebase_quick::single(snap, oid);
    entry(ui, t.tr("commits.menu.reword").into(), b.reword.as_deref(), QuickEdit::Reword(oid), msgs);
    entry(ui, t.tr("commits.menu.squash_parent").into(), b.squash.as_deref(), QuickEdit::SquashIntoParent(oid), msgs);
    preview(ui, t, b.drop.as_deref(), vec![oid], msgs);
}

/// Squash and the preview of dropping the selected commits, in `t`'s words (the menu's header
/// gives the count).
pub fn multi(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let b = rebase_quick::multi(snap, &m.oids);
    entry(ui, t.tr("commits.menu.squash").into(), b.squash.as_deref(), QuickEdit::Squash(m.oids.clone()), msgs);
    preview(ui, t, b.drop.as_deref(), m.oids.clone(), msgs);
}

/// Drop… for the commit `oid`, in the danger style: the commit menu's last item.
pub fn drop_single(ui: &mut egui::Ui, t: Lang, oid: Oid, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let blocked = rebase_quick::single(snap, oid).drop;
    drop_entry(ui, t, blocked.as_deref(), vec![oid], msgs);
}

/// Drop… for the selected commits, in the danger style: the multi-selection menu's last item.
pub fn drop_multi(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let blocked = rebase_quick::multi(snap, &m.oids).drop;
    drop_entry(ui, t, blocked.as_deref(), m.oids.clone(), msgs);
}

fn drop_entry(ui: &mut egui::Ui, t: Lang, blocked: Option<&str>, oids: Vec<Oid>, msgs: &mut Vec<Msg>) {
    entry_in(ui, t.tr("commits.menu.drop").into(), ItemState::danger_unless(blocked), QuickEdit::Drop(oids), msgs);
}

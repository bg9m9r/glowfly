//! The graph menus' history edits: reword, squash into the parent and drop one commit, or
//! squash and drop a multi-selection. Each opens a dialog or a confirmation, so each label ends
//! in an ellipsis, and each shows its key from the keymap (F2, Alt+S, Delete by default). Drop is
//! destructive: its menu puts it last, in the danger style, as a split row whose eye button
//! previews the drop. An entry that can't be used is greyed with an info mark, its reason on
//! hover or focus (`state::rebase_quick`); when reword, squash and drop are all blocked by one
//! reason, the commit menu shows one blocked row for the three (`state::commit_menu::menu_trim`) and no Drop.

use crate::keymap::{BindingId, Keymap};
use crate::msg::{Msg, OpPreviewMsg, RebaseMsg};
use crate::state::commit_menu::menu_trim::edits_blocked_alike;
use crate::state::rebase_quick::EditBlockers;
use crate::state::selection::MultiSelection;
use crate::ui::common::{menu_row, menu_row_preview};
use crate::ui::widgets::menu::item;
use crate::ui::widgets::reason::ItemState;
use eframe::egui;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::rebase::QuickEdit;

/// The key of `id` for a menu's shortcut column ("" without one).
pub fn key(keys: &Keymap, id: BindingId) -> &str {
    keys.hint(id).unwrap_or_default()
}

fn edit(e: QuickEdit) -> Msg {
    Msg::Rebase(RebaseMsg::Edit(e))
}

/// Reword and squash into parent for the commit `oid`, or, when the three edits are blocked
/// alike, one blocked row saying why.
pub fn single(ui: &mut egui::Ui, t: Lang, oid: Oid, b: &EditBlockers, keys: &Keymap, msgs: &mut Vec<Msg>) {
    if let Some(why) = edits_blocked_alike(b) {
        item(t.tr("commits.menu.edits_blocked")).blocked(Some(why)).show(ui);
        return;
    }
    let reword = item(t.tr("commits.menu.reword")).shortcut(key(keys, BindingId::RewordCommit)).blocked(b.reword.as_deref());
    menu_row(msgs, ui, reword, edit(QuickEdit::Reword(oid)));
    let squash = item(t.tr("commits.menu.squash_parent")).shortcut(key(keys, BindingId::SquashCommit)).blocked(b.squash.as_deref());
    menu_row(msgs, ui, squash, edit(QuickEdit::SquashIntoParent(oid)));
}

/// Squash the selected commits, in `t`'s words (the menu's header gives the count).
pub fn multi(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, b: &EditBlockers, keys: &Keymap, msgs: &mut Vec<Msg>) {
    let squash = item(t.tr("commits.menu.squash")).shortcut(key(keys, BindingId::SquashCommit)).blocked(b.squash.as_deref());
    menu_row(msgs, ui, squash, edit(QuickEdit::Squash(m.oids.clone())));
}

/// Whether the commit menu shows its own Drop row (not folded into the blocked edits row).
pub fn shows_drop(b: &EditBlockers) -> bool {
    edits_blocked_alike(b).is_none()
}

/// Drop… for `oids`, in the danger style, with the eye button previewing the drop (blocked as
/// Drop is): the commit and multi-selection menus' last item.
pub fn drop_row(ui: &mut egui::Ui, t: Lang, oids: &[Oid], b: &EditBlockers, keys: &Keymap, msgs: &mut Vec<Msg>) {
    let blocked = b.drop.as_deref();
    let row = item(t.tr("commits.menu.drop")).shortcut(key(keys, BindingId::DropCommit)).state(ItemState::danger_unless(blocked));
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Drop(oids.to_vec())));
    menu_row_preview(msgs, ui, row, (t.tr("op_preview.menu.drop"), None), edit(QuickEdit::Drop(oids.to_vec())), preview);
}

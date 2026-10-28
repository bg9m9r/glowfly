//! Right-click menus for commits and multi-selections, each headed by what it acts on, with
//! its items grouped by intent (the menu review's spec §10), operations with a preview as split
//! rows (the action, and an eye button that opens its preview), and the app's keys in the
//! shortcut column. A Copy submenu holds three or more copy items; fewer sit in the menu (local branches:
//! `branch_menu.rs`; remote branches: `remote_branch_menu.rs`; pull request items:
//! `pr_menu_items.rs`).

use super::common::{commit_menu_header, menu_header, menu_item, menu_item_if, menu_item_state, menu_note, menu_row, menu_row_preview};
use super::rebase::edit_menu::key;
use super::widgets::reason::{self, ItemState};
use crate::custom_commands::Target;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::keymap::{BindingId, Keymap};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::OpPreviewMsg;
use crate::msg::RebaseMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::commit_menu::menu_trim::head_contains;
use crate::state::commit_menu::{hard_reset_msg, reset_title};
use crate::state::custom_commands::MenuCommands;
use crate::state::op_preview::pick_blocker;
use crate::state::rebase_onto::{onto_commit_blocker, onto_menu_labels};
use crate::state::rebase_quick;
use crate::state::selection::{CommitBlock, MultiSelection, cherry_pick_blocker, in_graph_order};
use crate::tag::TagMsg;
use crate::ui::widgets::menu::{Named, cut_name, item, item_named, separator, submenu};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use glowfly_core::custom_commands::Place;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{CommitInfo, Snapshot};
use glowfly_core::worktree_ops::WorktreeBase;

/// The commit menu, in the review's groups: make something at the commit; apply it or move the
/// branch onto it (cherry-pick and rebase leave out when the branch already has it); edit it;
/// reset to it or rebase from it; compare, export and copy; the custom commands; and Drop.
/// Cherry-pick, rebase, the resets and Drop are split rows whose eye button previews them.
pub fn commit_menu(ui: &mut egui::Ui, t: Lang, c: &CommitInfo, snap: &Snapshot, cmds: &MenuCommands, keys: &Keymap, msgs: &mut Vec<Msg>) {
    let oid = c.oid;
    let sha = oid.to_string();
    let merge = c.parents.len() > 1;
    commit_menu_header(ui, &c.summary, oid);
    menu_item(msgs, ui, t.tr("commits.menu.checkout_detached"), Msg::git("Checkout", &["checkout", &sha]));
    let new_branch = Msg::Branch(BranchMsg::NewBranch { start: Some(sha.clone()), name: String::new() });
    menu_row(msgs, ui, item(t.tr("commits.menu.create_branch")).shortcut(key(keys, BindingId::CreateBranch)), new_branch);
    let tag = Msg::Tag(TagMsg::NewTag { at: sha.clone(), annotated: false });
    menu_row(msgs, ui, item(t.tr("commits.menu.create_tag")).shortcut(key(keys, BindingId::CreateTag)), tag);
    let worktree = Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(oid)));
    menu_item(msgs, ui, t.tr("commits.menu.create_worktree"), worktree);
    separator(ui);
    apply_items(ui, t, c, snap, msgs);
    separator(ui);
    let edits = rebase_quick::single(snap, oid);
    super::rebase::edit_menu::single(ui, t, oid, &edits, keys, msgs);
    separator(ui);
    reset_menu(ui, t, c, snap, msgs);
    let from_here = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(oid)));
    menu_item(msgs, ui, t.tr("commits.menu.interactive_from"), from_here);
    separator(ui);
    menu_item(msgs, ui, t.tr("commits.menu.compare_workdir"), Msg::Selection(SelectionMsg::CompareWithWorkdir(oid)));
    let patch_state = ItemState::blocked_by(merge.then(|| CommitBlock::PatchMerge.text(t)));
    menu_item_state(msgs, ui, t.tr("commits.menu.create_patch"), patch_state, Msg::Patch(PatchMsg::Export(vec![oid])));
    submenu(ui, t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("commits.menu.copy_sha"), Msg::Copy(sha.clone()));
        menu_item(msgs, ui, t.tr("commits.menu.copy_message"), Msg::Copy(c.summary.to_string()));
        menu_item(msgs, ui, t.tr("refs.copy.link"), Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::Commit(oid))));
        let blocked = merge.then(|| CommitBlock::PatchMerge.text(t));
        menu_item_if(msgs, ui, t.tr("commits.menu.copy_patch"), blocked, Msg::Patch(PatchMsg::Copy(vec![oid])));
    });
    super::custom_menu::group(ui, t, cmds, Place::Commit, Target::Commit(oid), msgs);
    if super::rebase::edit_menu::shows_drop(&edits) {
        separator(ui);
        super::rebase::edit_menu::drop_row(ui, t, &[oid], &edits, keys, msgs);
    }
}

/// Cherry-pick and revert, and rebasing the checked-out branch onto the commit (plainly or
/// interactively). Cherry-pick and the rebases leave out when the branch already contains the
/// commit: they can't apply to it.
fn apply_items(ui: &mut egui::Ui, t: Lang, c: &CommitInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let oid = c.oid;
    let merge = c.parents.len() > 1;
    let pick_block = cherry_pick_blocker(snap, &[oid]);
    if pick_block != Some(CommitBlock::PickOneOnHead) {
        let pick = if merge { t.tr("commits.menu.cherry_pick_merge") } else { t.tr("commits.menu.cherry_pick") };
        let preview_block = pick_blocker(t, snap, &[oid]);
        let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::CherryPick(vec![oid])));
        let row = item(pick).blocked(pick_block.map(|b| b.text(t)));
        let tip = (t.tr("op_preview.menu.cherry_pick"), preview_block.as_deref());
        menu_row_preview(msgs, ui, row, tip, Msg::Selection(SelectionMsg::CherryPick(vec![oid])), preview);
    }
    let revert = if merge { t.tr("commits.menu.revert_merge") } else { t.tr("commits.menu.revert") };
    menu_item(msgs, ui, revert, Msg::Selection(SelectionMsg::Revert(vec![oid])));
    let Some(current) = snap.head_branch.as_deref() else { return };
    if head_contains(snap, oid) {
        return;
    }
    let sha = oid.to_string();
    let blocked = onto_commit_blocker(t, snap, oid);
    let (plain, interactive) = onto_menu_labels(t, &cut_name(current));
    let (plain_full, interactive_full) = onto_menu_labels(t, current);
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Rebase { onto: sha.clone(), oid }));
    let row = item_named(Named { shown: plain, full: plain_full }).blocked(blocked.as_deref());
    menu_row_preview(msgs, ui, row, (t.tr("op_preview.menu.rebase"), None), Msg::git("Rebase", &["rebase", &sha]), preview);
    let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
    menu_row(msgs, ui, item_named(Named { shown: interactive, full: interactive_full }).blocked(blocked.as_deref()), onto);
}

/// The reset submenu: soft, mixed and hard (asked first), titled with the branch it moves. Each
/// is a split row whose eye button opens the reset's preview (which offers all three).
fn reset_menu(ui: &mut egui::Ui, t: Lang, c: &CommitInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let sha = c.oid.to_string();
    let current = snap.head_branch.as_deref().map(cut_name);
    submenu(ui, reset_title(t, current.as_deref()), |ui| {
        let preview = || Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Reset { to: c.oid }));
        let tip = (t.tr("op_preview.menu.reset"), None);
        let soft = Msg::git("Reset (soft)", &["reset", "--soft", &sha]);
        menu_row_preview(msgs, ui, item(t.tr("commits.menu.reset.soft")), tip, soft, preview());
        let mixed = Msg::git("Reset (mixed)", &["reset", "--mixed", &sha]);
        menu_row_preview(msgs, ui, item(t.tr("commits.menu.reset.mixed")), tip, mixed, preview());
        let hard = item(t.tr("commits.menu.reset.hard")).state(ItemState::Danger);
        menu_row_preview(msgs, ui, hard, tip, hard_reset_msg(t, snap, c), preview());
        menu_note(ui, t.tr("commits.menu.reset.hard_hint"));
    });
}

/// The menu for a row of a multi-selection, headed by how many commits there are: apply them
/// or squash them; compare, export and copy them; and Drop. Cherry-pick and Drop are split rows
/// whose eye button previews them.
pub fn multi_commit_menu(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, keys: &Keymap, msgs: &mut Vec<Msg>) {
    menu_header(ui, &t.trn("details.multi.selected", m.len(), &[]));
    let oldest_first = in_graph_order(m.oids.iter().copied(), snap, true);
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::CherryPick(oldest_first)));
    let blocked = m.pick_blocked.map(|b| b.text(t));
    let row = item(t.tr("commits.menu.cherry_pick")).blocked(blocked);
    let tip = (t.tr("op_preview.menu.cherry_pick"), blocked);
    menu_row_preview(msgs, ui, row, tip, Msg::Selection(SelectionMsg::CherryPick(m.oids.clone())), preview);
    let reverted = revert_button(ui, t, t.tr("commits.menu.revert"), m, true, msgs);
    let edits = rebase_quick::multi(snap, &m.oids);
    super::rebase::edit_menu::multi(ui, t, m, &edits, keys, msgs);
    separator(ui);
    range_diff_item(ui, t, m, msgs);
    let patched = patch_button(ui, t, t.tr("commits.menu.create_patch"), m, true, msgs);
    if reverted || patched {
        ui.close_menu();
    }
    menu_item(msgs, ui, t.tr("commits.menu.copy_shas_item"), copy_shas(m));
    let copy = Msg::Patch(PatchMsg::Copy(m.oids.clone()));
    menu_item_if(msgs, ui, t.tr("commits.menu.copy_patch_item"), m.export_blocked.map(|b| b.text(t)), copy);
    separator(ui);
    super::rebase::edit_menu::drop_row(ui, t, &m.oids, &edits, keys, msgs);
}

/// Range-diff the two commits: the older row (graph order is newest first) is the old version.
/// Disabled unless exactly two are selected.
fn range_diff_item(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, msgs: &mut Vec<Msg>) {
    let label = t.tr("commits.menu.range_diff");
    match (crate::state::range_diff::multi_blocked(t, m), m.oids.as_slice()) {
        (None, [new, old]) => {
            let open = crate::range_diff::RangeDiffMsg::Open { old: *old, new: *new, label: None, at: None };
            menu_item(msgs, ui, label, Msg::RangeDiff(open));
        }
        (why, _) => {
            let why = why.unwrap_or(t.tr("range_diff.blocked.two"));
            reason::menu_button(ui, label, "", ItemState::blocked_by(Some(why)));
        }
    }
}

/// A multi-selection action: in a menu (`in_menu`), an item greyed with an info mark while it is
/// disabled, its reason on hover or focus; in the selection panel, a plain button with the
/// reason on hover.
fn multi_action(
    ui: &mut egui::Ui,
    t: Lang,
    label: impl Into<egui::WidgetText>,
    blocked: Option<CommitBlock>,
    in_menu: bool,
) -> egui::Response {
    let blocked = blocked.map(|b| b.text(t));
    if in_menu {
        return reason::menu_button(ui, label, "", ItemState::blocked_by(blocked));
    }
    let r = ui.add_enabled(blocked.is_none(), egui::Button::new(label));
    match blocked {
        Some(why) => r.on_disabled_hover_text(why),
        None => r,
    }
}

/// Cherry-pick the selected commits, disabled with the reason when blocked. Returns
/// whether it was clicked.
pub(super) fn cherry_pick_button(
    ui: &mut egui::Ui,
    t: Lang,
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, t, label, m.pick_blocked, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::CherryPick(m.oids.clone())));
    }
    clicked
}

/// Revert the selected commits (merges among them ask for their mainline first). Returns
/// whether it was clicked.
pub(super) fn revert_button(
    ui: &mut egui::Ui,
    t: Lang,
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, t, label, None, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::Revert(m.oids.clone())));
    }
    clicked
}

/// Create one patch from the selected commits, disabled with the reason when a merge
/// is selected. Returns whether it was clicked.
pub(super) fn patch_button(
    ui: &mut egui::Ui,
    t: Lang,
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, t, label, m.export_blocked, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Patch(PatchMsg::Export(m.oids.clone())));
    }
    clicked
}

/// Copies the selected commits' full SHAs, one per line, newest first.
pub(super) fn copy_shas(m: &MultiSelection) -> Msg {
    Msg::Copy(crate::state::menu_bar::shas_text(m))
}

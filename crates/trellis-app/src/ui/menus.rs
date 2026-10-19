//! Right-click menus for commits and multi-selections, each headed by what it acts on, with
//! its items grouped by intent and its copy items in one Copy submenu (local branches:
//! `branch_menu.rs`; remote branches: `remote_branch_menu.rs`; pull request items:
//! `pr_menu_items.rs`).

use super::common::{commit_menu_header, menu_header, menu_item, menu_item_if, menu_note};
use super::widgets::reason::{self, ItemState};
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::commit_menu::{hard_reset_body, reset_title};
use crate::state::patch::PATCH_BLOCKED_MERGE;
use crate::state::rebase::needs_stash;
use crate::state::rebase_onto::{onto_commit_blocker, onto_menu_labels};
use crate::state::selection::{MultiSelection, cherry_pick_blocker};
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use eframe::egui;
use trellis_core::i18n::Lang;
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{CommitInfo, Snapshot};
use trellis_core::worktree_ops::WorktreeBase;

pub fn commit_menu(ui: &mut egui::Ui, t: Lang, c: &CommitInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let oid = c.oid;
    let sha = oid.to_string();
    let merge = c.parents.len() > 1;
    commit_menu_header(ui, &c.summary, oid);
    menu_item(msgs, ui, t.tr("commits.menu.checkout_detached"), Msg::git("Checkout", &["checkout", &sha]));
    let new_branch = Msg::Branch(BranchMsg::NewBranch { start: Some(sha.clone()), name: String::new() });
    menu_item(msgs, ui, t.tr("commits.menu.create_branch"), new_branch);
    menu_item(msgs, ui, t.tr("commits.menu.create_tag"), Msg::Tag(TagMsg::NewTag { at: sha.clone(), annotated: false }));
    let worktree = Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(oid)));
    menu_item(msgs, ui, t.tr("commits.menu.create_worktree"), worktree);
    ui.separator();
    let pick = if merge { t.tr("commits.menu.cherry_pick_merge") } else { t.tr("commits.menu.cherry_pick") };
    menu_item_if(msgs, ui, pick, cherry_pick_blocker(snap, &[oid]), Msg::Selection(SelectionMsg::CherryPick(vec![oid])));
    let revert = if merge { t.tr("commits.menu.revert_merge") } else { t.tr("commits.menu.revert") };
    menu_item(msgs, ui, revert, Msg::Selection(SelectionMsg::Revert(vec![oid])));
    if let Some(current) = snap.head_branch.as_deref() {
        let blocked = onto_commit_blocker(snap, oid);
        let (plain, interactive) = onto_menu_labels(t, current);
        menu_item_if(msgs, ui, plain, blocked.as_deref(), Msg::git("Rebase", &["rebase", &sha]));
        let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
        menu_item_if(msgs, ui, interactive, blocked.as_deref(), onto);
    }
    ui.separator();
    reset_menu(ui, t, c, snap, msgs);
    let from_here = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(oid)));
    menu_item(msgs, ui, t.tr("commits.menu.interactive_from"), from_here);
    super::rebase::edit_menu::single(ui, t, oid, snap, msgs);
    ui.separator();
    menu_item(msgs, ui, t.tr("commits.menu.compare_workdir"), Msg::Selection(SelectionMsg::CompareWithWorkdir(oid)));
    ui.separator();
    let patch_state = ItemState::blocked_by(merge.then_some(PATCH_BLOCKED_MERGE));
    let patch = reason::menu_button(ui, t.tr("commits.menu.create_patch"), "", patch_state);
    if patch.clicked() {
        msgs.push(Msg::Patch(PatchMsg::Export(vec![oid])));
        ui.close_menu();
    }
    ui.menu_button(t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("commits.menu.copy_sha"), Msg::Copy(sha.clone()));
        menu_item(msgs, ui, t.tr("commits.menu.copy_message"), Msg::Copy(c.summary.clone()));
        menu_item(msgs, ui, t.tr("refs.copy.link"), Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::Commit(oid))));
        let blocked = merge.then_some(PATCH_BLOCKED_MERGE);
        menu_item_if(msgs, ui, t.tr("commits.menu.copy_patch"), blocked, Msg::Patch(PatchMsg::Copy(vec![oid])));
    });
}

/// The reset submenu: soft, mixed and hard (asked first), titled with the branch it moves.
fn reset_menu(ui: &mut egui::Ui, t: Lang, c: &CommitInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let sha = c.oid.to_string();
    let current = snap.head_branch.as_deref();
    ui.menu_button(reset_title(t, current), |ui| {
        menu_item(msgs, ui, t.tr("commits.menu.reset.soft"), Msg::git("Reset (soft)", &["reset", "--soft", &sha]));
        menu_item(msgs, ui, t.tr("commits.menu.reset.mixed"), Msg::git("Reset (mixed)", &["reset", "--mixed", &sha]));
        let hard = Msg::git("Reset (hard)", &["reset", "--hard", &sha]);
        let body = hard_reset_body(t, current, c);
        // With uncommitted changes the auto-stash prompt asks instead, so it asks once.
        let ask = if needs_stash(snap) {
            hard
        } else {
            Msg::confirm_danger(t.tr("commits.menu.reset.hard_title"), body, t.tr("commits.menu.reset.button"), hard)
        };
        menu_item(msgs, ui, t.tr("commits.menu.reset.hard"), ask);
        menu_note(ui, t.tr("commits.menu.reset.hard_hint"));
    });
}

/// The menu for a row of a multi-selection: actions on all the selected commits, headed by how
/// many there are.
pub fn multi_commit_menu(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    menu_header(ui, &t.trn("details.multi.selected", m.len(), &[]));
    let picked = cherry_pick_button(ui, t.tr("commits.menu.cherry_pick"), m, true, msgs);
    let reverted = revert_button(ui, t.tr("commits.menu.revert"), m, true, msgs);
    ui.separator();
    super::rebase::edit_menu::multi(ui, t, m, snap, msgs);
    ui.separator();
    let patched = patch_button(ui, t.tr("commits.menu.create_patch"), m, true, msgs);
    if picked || reverted || patched {
        ui.close_menu();
    }
    ui.menu_button(t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("commits.menu.copy_shas"), copy_shas(m));
        let copy = Msg::Patch(PatchMsg::Copy(m.oids.clone()));
        menu_item_if(msgs, ui, t.tr("commits.menu.copy_patch"), m.export_blocked, copy);
    });
}

/// A multi-selection action: in a menu (`in_menu`), an item with a short form of why it is
/// disabled after its label; in the selection panel, a plain button with the reason on hover.
fn multi_action(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, blocked: Option<&str>, in_menu: bool) -> egui::Response {
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
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, label, m.pick_blocked, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::CherryPick(m.oids.clone())));
    }
    clicked
}

/// Revert the selected commits, disabled with the reason when a merge is selected.
/// Returns whether it was clicked.
pub(super) fn revert_button(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, label, m.revert_blocked, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::Revert(m.oids.clone())));
    }
    clicked
}

/// Create one patch from the selected commits, disabled with the reason when a merge
/// is selected. Returns whether it was clicked.
pub(super) fn patch_button(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    m: &MultiSelection,
    in_menu: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let clicked = multi_action(ui, label, m.export_blocked, in_menu).clicked();
    if clicked {
        msgs.push(Msg::Patch(PatchMsg::Export(m.oids.clone())));
    }
    clicked
}

/// Copies the selected commits' full SHAs, one per line, newest first.
pub(super) fn copy_shas(m: &MultiSelection) -> Msg {
    Msg::Copy(m.oids.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(
        "
",
    ))
}

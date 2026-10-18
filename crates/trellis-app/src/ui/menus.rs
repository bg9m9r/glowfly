//! Right-click menus for commits and multi-selections (local branches: `branch_menu.rs`; remote
//! branches: `remote_branch_menu.rs`; pull request items: `pr_menu_items.rs`).

use super::common::{menu_item, menu_item_if, short};
use super::theme::color;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::patch::PATCH_BLOCKED_MERGE;
use crate::state::rebase_onto::{onto_commit_blocker, onto_labels};
use crate::state::selection::{MultiSelection, cherry_pick_blocker};
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{CommitInfo, Snapshot};
use trellis_core::worktree_ops::WorktreeBase;

pub fn commit_menu(ui: &mut egui::Ui, c: &CommitInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let oid = c.oid;
    let sha = oid.to_string();
    let s = short(oid);
    menu_item(msgs, ui, "Create branch here…", Msg::Branch(BranchMsg::NewBranch { start: Some(sha.clone()), name: String::new() }));
    menu_item(msgs, ui, "Create tag here…", Msg::Tag(TagMsg::NewTag { at: sha.clone(), annotated: false }));
    menu_item(msgs, ui, "Create worktree from this commit…", Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(oid))));
    menu_item(msgs, ui, "Checkout this commit (detached)", Msg::git("Checkout", &["checkout", &sha]));
    menu_item(msgs, ui, "Compare with working directory", Msg::Selection(SelectionMsg::CompareWithWorkdir(oid)));
    ui.separator();
    let merge = c.parents.len() > 1;
    let pick = if merge { "Cherry-pick merge commit…" } else { "Cherry-pick commit" };
    menu_item_if(msgs, ui, pick, cherry_pick_blocker(snap, &[oid]), Msg::Selection(SelectionMsg::CherryPick(vec![oid])));
    let revert = if merge { "Revert merge commit…" } else { "Revert commit" };
    menu_item(msgs, ui, revert, Msg::Selection(SelectionMsg::Revert(vec![oid])));
    ui.menu_button("Reset current branch to this commit", |ui| {
        menu_item(msgs, ui, "Soft - keep all changes staged", Msg::git("Reset (soft)", &["reset", "--soft", &sha]));
        menu_item(msgs, ui, "Mixed - keep working copy, reset index", Msg::git("Reset (mixed)", &["reset", "--mixed", &sha]));
        menu_item(
            msgs,
            ui,
            "Hard - discard all changes",
            Msg::confirm(
                "Hard reset",
                format!(
                    "Reset the current branch to {s}? Uncommitted changes to tracked files are stashed first, so Undo brings them back."
                ),
                "Reset",
                Msg::git("Reset (hard)", &["reset", "--hard", &sha]),
            ),
        );
    });
    menu_item(msgs, ui, "Interactive rebase from here…", Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(oid))));
    if let Some(current) = snap.head_branch.as_deref() {
        let blocked = onto_commit_blocker(snap, oid);
        let (plain, interactive) = onto_labels(current, oid);
        menu_item_if(msgs, ui, plain, blocked.as_deref(), Msg::git("Rebase", &["rebase", &sha]));
        let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
        menu_item_if(msgs, ui, interactive, blocked.as_deref(), onto);
    }
    super::rebase::edit_menu::single(ui, oid, snap, msgs);
    ui.separator();
    let patch = ui.add_enabled(c.parents.len() < 2, egui::Button::new("Create patch…")).on_disabled_hover_text(PATCH_BLOCKED_MERGE);
    if patch.clicked() {
        msgs.push(Msg::Patch(PatchMsg::Export(vec![oid])));
        ui.close_menu();
    }
    let copy = ui.add_enabled(c.parents.len() < 2, egui::Button::new("Copy patch")).on_disabled_hover_text(PATCH_BLOCKED_MERGE);
    if copy.clicked() {
        msgs.push(Msg::Patch(PatchMsg::Copy(vec![oid])));
        ui.close_menu();
    }
    menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(sha.clone()));
    menu_item(msgs, ui, "Copy commit message", Msg::Copy(c.summary.clone()));
    menu_item(msgs, ui, "Copy link to this commit", Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::Commit(oid))));
}

/// The menu for a row of a multi-selection: actions on all the selected commits.
pub fn multi_commit_menu(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    ui.label(RichText::new(t.trn("details.multi.selected", m.len(), &[])).color(color::weak()));
    ui.separator();
    let picked = cherry_pick_button(ui, t, m, msgs);
    let reverted = revert_button(ui, t, m, msgs);
    let patched = patch_button(ui, t, m, msgs);
    let copy = egui::Button::new(t.trn("details.multi.copy_patch.button", m.len(), &[]));
    let mut r = ui.add_enabled(m.export_blocked.is_none(), copy);
    if let Some(why) = m.export_blocked {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked() {
        msgs.push(Msg::Patch(PatchMsg::Copy(m.oids.clone())));
    }
    if picked || reverted || patched || r.clicked() {
        ui.close_menu();
    }
    super::rebase::edit_menu::multi(ui, m, snap, msgs);
    menu_item(msgs, ui, t.tr("details.multi.copy_shas.button"), copy_shas(m));
}

/// "Cherry-pick N commits", disabled with the reason on hover when blocked. Returns whether it
/// was clicked.
pub(super) fn cherry_pick_button(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(t.trn("details.multi.cherry_pick.button", m.len(), &[]));
    let mut r = ui.add_enabled(m.pick_blocked.is_none(), button);
    if let Some(why) = m.pick_blocked {
        r = r.on_disabled_hover_text(why);
    }
    let clicked = r.clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::CherryPick(m.oids.clone())));
    }
    clicked
}

/// "Revert N commits", disabled with the reason on hover when a merge is selected. Returns
/// whether it was clicked.
pub(super) fn revert_button(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(t.trn("details.multi.revert.button", m.len(), &[]));
    let mut r = ui.add_enabled(m.revert_blocked.is_none(), button);
    if let Some(why) = m.revert_blocked {
        r = r.on_disabled_hover_text(why);
    }
    let clicked = r.clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::Revert(m.oids.clone())));
    }
    clicked
}

/// "Create patch from N commits…", disabled with the reason on hover when a merge is selected.
/// Returns whether it was clicked.
pub(super) fn patch_button(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(t.trn("details.multi.patch.button", m.len(), &[]));
    let mut r = ui.add_enabled(m.export_blocked.is_none(), button);
    if let Some(why) = m.export_blocked {
        r = r.on_disabled_hover_text(why);
    }
    let clicked = r.clicked();
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

//! Right-click menus for commits and remote branches (local branches: `branch_menu.rs`; pull
//! request items: `pr_menu_items.rs`).

use super::common::{menu_item, short};
use super::ref_visibility;
use super::theme::color;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::patch::PATCH_BLOCKED_MERGE;
use crate::state::selection::MultiSelection;
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, RichText};
use git2::Oid;
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
    ui.separator();
    menu_item(msgs, ui, "Cherry-pick commit", Msg::git("Cherry-pick", &["cherry-pick", &sha]));
    menu_item(msgs, ui, "Revert commit", Msg::git("Revert", &["revert", "--no-edit", &sha]));
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
pub fn multi_commit_menu(ui: &mut egui::Ui, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    ui.label(RichText::new(format!("{} commits selected", m.len())).color(color::weak()));
    ui.separator();
    let picked = cherry_pick_button(ui, m, msgs);
    let patched = patch_button(ui, m, msgs);
    let copy = egui::Button::new(format!("Copy patch of {} commits", m.len()));
    let mut r = ui.add_enabled(m.export_blocked.is_none(), copy);
    if let Some(why) = m.export_blocked {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked() {
        msgs.push(Msg::Patch(PatchMsg::Copy(m.oids.clone())));
    }
    if picked || patched || r.clicked() {
        ui.close_menu();
    }
    super::rebase::edit_menu::multi(ui, m, snap, msgs);
    menu_item(msgs, ui, "Copy SHAs", copy_shas(m));
}

/// "Cherry-pick N commits", disabled with the reason on hover when blocked. Returns whether it
/// was clicked.
pub(super) fn cherry_pick_button(ui: &mut egui::Ui, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(format!("Cherry-pick {} commits", m.len()));
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

/// "Create patch from N commits…", disabled with the reason on hover when a merge is selected.
/// Returns whether it was clicked.
pub(super) fn patch_button(ui: &mut egui::Ui, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(format!("Create patch from {} commits…", m.len()));
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

pub fn remote_branch_menu(
    ui: &mut egui::Ui,
    remote: &str,
    name: &str,
    oid: Oid,
    snap: &Snapshot,
    c: &super::remotes::Ctx,
    msgs: &mut Vec<Msg>,
) {
    let (current, fs, filter) = (c.current, c.fs, c.graph);
    let full = format!("{remote}/{name}");
    menu_item(msgs, ui, format!("Checkout {full}"), Msg::git("Checkout", &["checkout", "--track", &full]));
    if name != "HEAD" {
        let base = WorktreeBase::Remote { remote: remote.to_owned(), branch: name.to_owned() };
        menu_item(msgs, ui, format!("Create worktree from {full}…"), Msg::Worktree(WorktreeMsg::CreateDialog(base)));
    }
    if !current.is_empty() {
        menu_item(msgs, ui, format!("Merge {full} into {current}"), Msg::git("Merge", &["merge", &full]));
        menu_item(msgs, ui, format!("Rebase {current} onto {full}"), Msg::git("Rebase", &["rebase", &full]));
    }
    if let Some(fs) = fs.filter(|f| f.remote == remote && f.host.is_some()) {
        ui.separator();
        match fs.pr_for_branch(name) {
            Some(pr) => menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone())),
            None => super::pr_menu_items::remote(ui, name, oid, snap, fs, c.sel, msgs),
        }
    }
    menu_item(msgs, ui, "Copy branch name", Msg::Copy(full.clone()));
    let link = LinkTarget::RemoteBranch { remote: remote.to_owned(), name: name.to_owned() };
    menu_item(msgs, ui, format!("Copy link to branch {full}"), Msg::DeepLink(DeepLinkMsg::Copy(link)));
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/remotes/{full}"), &full, filter, false, msgs);
    ui.separator();
    menu_item(
        msgs,
        ui,
        format!("Delete {full} from remote"),
        Msg::confirm(
            "Delete remote branch",
            format!("Delete branch {name} on {remote}? This affects everyone using the remote."),
            "Delete",
            Msg::git("Delete remote branch", &["push", remote, "--delete", name]),
        ),
    );
}

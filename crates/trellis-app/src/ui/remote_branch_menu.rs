//! The right-click menu of a remote-tracking branch, shared by the REMOTE rows of the left panel
//! and the remote-branch labels in the graph: checkout (the local branch tracking it, or a new
//! one), fast-forward of that local branch, a worktree, merge, rebase, conflict check, compare
//! and set upstream against the checked-out branch, pull requests, a new branch, copy, hide or
//! solo, and delete on the remote. Disabled items say why on hover, with the same text `update`
//! logs when it refuses a late message.

use super::common::{menu_item, menu_item_if};
use super::ref_visibility;
use crate::conflict_check::ConflictCheckMsg;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::{BranchMsg, Msg, RebaseMsg, SelectionMsg};
use crate::state::rebase_onto::onto_commit_blocker;
use crate::state::{ForgeState, Selection, branches, remote_branch, worktree_branches};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use git2::Oid;
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::Snapshot;
use trellis_core::settings::GraphFilter;
use trellis_core::worktree_ops::WorktreeBase;

/// What a remote branch's menu is drawn against.
pub struct RemoteMenu<'a> {
    /// The checked-out branch's name ("" when detached).
    pub current: &'a str,
    pub fs: Option<&'a ForgeState>,
    pub graph: &'a GraphFilter,
    pub sel: &'a Selection,
}

pub fn remote_branch_menu(ui: &mut egui::Ui, remote: &str, name: &str, oid: Oid, snap: &Snapshot, c: &RemoteMenu, msgs: &mut Vec<Msg>) {
    let (current, fs, filter) = (c.current, c.fs, c.graph);
    let full = format!("{remote}/{name}");
    local_items(ui, remote, name, snap, msgs);
    if !current.is_empty() {
        current_items(ui, remote, name, oid, snap, current, msgs);
    }
    if let Some(fs) = fs.filter(|f| f.remote == remote && f.host.is_some()) {
        ui.separator();
        match fs.pr_for_branch(name) {
            Some(pr) => menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone())),
            None => super::pr_menu_items::remote(ui, name, oid, snap, fs, c.sel, msgs),
        }
    }
    ui.separator();
    menu_item(msgs, ui, "Create branch here…", Msg::Branch(BranchMsg::NewBranch { start: Some(oid.to_string()), name: String::new() }));
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

/// The local counterpart: check out the tracking branch (or create one), fast-forward it, and
/// create a worktree.
fn local_items(ui: &mut egui::Ui, remote: &str, name: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let full = format!("{remote}/{name}");
    match remote_branch::tracking_local(snap, remote, name) {
        Some(b) => {
            let local = b.name.as_str();
            if !b.is_head {
                let blocked = worktree_branches::checkout_blocked(snap, local);
                menu_item_if(msgs, ui, format!("Checkout {local}"), blocked.as_deref(), Msg::Branch(BranchMsg::Checkout(local.to_owned())));
            }
            let blocked = branches::fast_forward_blocked(b, snap.state);
            let ff = Msg::Branch(BranchMsg::FastForwardBranch(local.to_owned()));
            menu_item_if(msgs, ui, format!("Fast-forward {local} to {full}"), blocked.as_deref(), ff);
        }
        None => {
            let blocked = remote_branch::checkout_blocked(snap, remote, name);
            let checkout = Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.to_owned(), name: name.to_owned() });
            menu_item_if(msgs, ui, format!("Checkout {full}"), blocked.as_deref(), checkout);
        }
    }
    if name != "HEAD" {
        let base = WorktreeBase::Remote { remote: remote.to_owned(), branch: name.to_owned() };
        menu_item(msgs, ui, format!("Create worktree from {full}…"), Msg::Worktree(WorktreeMsg::CreateDialog(base)));
    }
}

/// Operations against the checked-out branch `current`: merge, rebase, conflict check, compare
/// and set upstream.
fn current_items(ui: &mut egui::Ui, remote: &str, name: &str, oid: Oid, snap: &Snapshot, current: &str, msgs: &mut Vec<Msg>) {
    let full = format!("{remote}/{name}");
    menu_item(msgs, ui, format!("Merge {full} into {current}"), Msg::git("Merge", &["merge", &full]));
    menu_item(msgs, ui, format!("Rebase {current} onto {full}"), Msg::git("Rebase", &["rebase", &full]));
    let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
    let blocked = onto_commit_blocker(snap, oid);
    menu_item_if(msgs, ui, format!("Interactive rebase {current} onto {full}…"), blocked.as_deref(), onto);
    let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(full.clone()));
    menu_item(msgs, ui, format!("Check for conflicts with {current}"), check);
    let compare = Msg::Selection(SelectionMsg::CompareWithHead { oid, name: full.clone() });
    let blocked = remote_branch::compare_blocked(snap, oid, &full);
    menu_item_if(msgs, ui, format!("Compare {full} with {current}"), blocked.as_deref(), compare);
    if !remote_branch::is_upstream_of_head(snap, remote, name) {
        let set = BranchMsg::SetUpstream { branch: current.to_owned(), remote: remote.to_owned(), name: name.to_owned() };
        menu_item(msgs, ui, format!("Set {full} as upstream of {current}"), Msg::Branch(set));
    }
}

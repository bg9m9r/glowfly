//! The right-click menu of a remote-tracking branch, shared by the REMOTE rows of the left panel
//! and the remote-branch labels in the graph, in groups: checkout (the local branch tracking it,
//! or a new one), a new branch and a worktree; fast-forward of that local branch, merge, rebase,
//! conflict check, compare and set upstream against the checked-out branch; pull requests; hide
//! or solo; the Copy submenu; and delete on the remote. The branch's name heads the menu once;
//! the items leave it out. Disabled items say why on hover, with the same text `update` logs
//! when it refuses a late message.

use super::common::{menu_header, menu_item, menu_item_if};
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
use trellis_core::repo::{BranchInfo, Snapshot};
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
    menu_header(ui, &full);
    let tracking = remote_branch::tracking_local(snap, remote, name);
    checkout_items(ui, remote, name, oid, tracking, snap, msgs);
    if tracking.is_some() || !current.is_empty() {
        ui.separator();
    }
    if let Some(b) = tracking {
        let blocked = branches::fast_forward_blocked(b, snap.state);
        let ff = Msg::Branch(BranchMsg::FastForwardBranch(b.name.clone()));
        menu_item_if(msgs, ui, format!("Fast-forward {} to this branch", b.name), blocked.as_deref(), ff);
    }
    if !current.is_empty() {
        current_items(ui, remote, name, oid, snap, current, msgs);
    }
    match fs.filter(|f| f.host.is_some()) {
        Some(fs) if fs.remote == remote => {
            ui.separator();
            match fs.pr_for_branch(name) {
                Some(pr) => menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone())),
                None => super::pr_menu_items::remote(ui, name, oid, snap, fs, c.sel, msgs),
            }
        }
        // Another remote of a repository on a code host: pull requests go through the host's.
        Some(_) => {}
        None => {
            ui.separator();
            super::pr_menu_items::remote_no_host(ui, remote, name, snap, msgs);
        }
    }
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/remotes/{full}"), filter, false, msgs);
    ui.separator();
    ui.menu_button("Copy", |ui| {
        menu_item(msgs, ui, "Name", Msg::Copy(full.clone()));
        let link = LinkTarget::RemoteBranch { remote: remote.to_owned(), name: name.to_owned() };
        menu_item(msgs, ui, "Link", Msg::DeepLink(DeepLinkMsg::Copy(link)));
    });
    ui.separator();
    menu_item(
        msgs,
        ui,
        "Delete from remote…",
        Msg::confirm_danger(
            "Delete remote branch",
            format!("Delete branch {name} on {remote}? This affects everyone using the remote."),
            "Delete",
            Msg::git("Delete remote branch", &["push", remote, "--delete", name]),
        ),
    );
}

/// Check out the tracking branch (or create one), create a branch here, and create a worktree.
fn checkout_items(
    ui: &mut egui::Ui,
    remote: &str,
    name: &str,
    oid: Oid,
    tracking: Option<&BranchInfo>,
    snap: &Snapshot,
    msgs: &mut Vec<Msg>,
) {
    match tracking {
        Some(b) if !b.is_head => {
            let local = b.name.as_str();
            let blocked = worktree_branches::checkout_blocked(snap, local);
            menu_item_if(msgs, ui, format!("Checkout {local}"), blocked.as_deref(), Msg::Branch(BranchMsg::Checkout(local.to_owned())));
        }
        Some(_) => {}
        None => {
            let blocked = remote_branch::checkout_blocked(snap, remote, name);
            let checkout = Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.to_owned(), name: name.to_owned() });
            menu_item_if(msgs, ui, "Checkout", blocked.as_deref(), checkout);
        }
    }
    menu_item(msgs, ui, "Create branch here…", Msg::Branch(BranchMsg::NewBranch { start: Some(oid.to_string()), name: String::new() }));
    if name != "HEAD" {
        let base = WorktreeBase::Remote { remote: remote.to_owned(), branch: name.to_owned() };
        menu_item(msgs, ui, "Create worktree from this branch…", Msg::Worktree(WorktreeMsg::CreateDialog(base)));
    }
}

/// Operations against the checked-out branch `current`: merge, rebase, conflict check, compare
/// and set upstream.
fn current_items(ui: &mut egui::Ui, remote: &str, name: &str, oid: Oid, snap: &Snapshot, current: &str, msgs: &mut Vec<Msg>) {
    let full = format!("{remote}/{name}");
    menu_item(msgs, ui, format!("Merge into {current}"), Msg::git("Merge", &["merge", &full]));
    menu_item(msgs, ui, format!("Rebase {current} onto this branch"), Msg::git("Rebase", &["rebase", &full]));
    let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
    let blocked = onto_commit_blocker(snap, oid);
    menu_item_if(msgs, ui, format!("Interactive rebase {current} onto this branch…"), blocked.as_deref(), onto);
    let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(full.clone()));
    menu_item(msgs, ui, format!("Check for conflicts with {current}"), check);
    let compare = Msg::Selection(SelectionMsg::CompareWithHead { oid, name: full.clone() });
    let blocked = remote_branch::compare_blocked(snap, oid, &full);
    menu_item_if(msgs, ui, format!("Compare with {current}"), blocked.as_deref(), compare);
    if !remote_branch::is_upstream_of_head(snap, remote, name) {
        let set = BranchMsg::SetUpstream { branch: current.to_owned(), remote: remote.to_owned(), name: name.to_owned() };
        menu_item(msgs, ui, format!("Set as upstream of {current}"), Msg::Branch(set));
    }
}

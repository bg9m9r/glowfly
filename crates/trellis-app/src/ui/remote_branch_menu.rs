//! The right-click menu of a remote-tracking branch, shared by the REMOTE rows of the left panel
//! and the remote-branch labels in the graph, in groups: checkout (the local branch tracking it,
//! or a new one), a new branch and a worktree; fast-forward of that local branch, merge, rebase,
//! conflict check, compare and set upstream against the checked-out branch, and comparing it
//! with its version before the last force-push (Range-diff); pull requests; hide
//! or solo, and its colour; the Copy submenu; and delete on the remote. The branch's name heads
//! the menu once; the items leave it out. Disabled items say why on hover, with the same text
//! `update` logs when it refuses a late message.

use super::common::{menu_header, menu_item, menu_item_if};
use super::ref_visibility;
use crate::conflict_check::ConflictCheckMsg;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::{BranchMsg, Msg, OpPreviewMsg, RebaseMsg, SelectionMsg};
use crate::state::gitflow_base::{Deleting, guard};
use crate::state::rebase_onto::onto_commit_blocker;
use crate::state::{ForgeState, Selection, branches, remote_branch, worktree_branches};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::op_preview::PreviewOp;
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
    pub t: Lang,
}

pub fn remote_branch_menu(ui: &mut egui::Ui, remote: &str, name: &str, oid: Oid, snap: &Snapshot, c: &RemoteMenu, msgs: &mut Vec<Msg>) {
    let (current, fs, filter, t) = (c.current, c.fs, c.graph, c.t);
    let full = format!("{remote}/{name}");
    menu_header(ui, &full);
    let tracking = remote_branch::tracking_local(snap, remote, name);
    checkout_items(ui, t, (remote, name), oid, tracking, snap, msgs);
    ui.separator();
    if let Some(b) = tracking {
        let blocked = branches::fast_forward_blocked(b, snap.state);
        let ff = Msg::Branch(BranchMsg::FastForwardBranch(b.name.clone()));
        let label = t.trf("remote_branch.menu.fast_forward_local", &[("branch", &b.name)]);
        menu_item_if(msgs, ui, label, blocked.as_deref(), ff);
    }
    if !current.is_empty() {
        current_items(ui, c, (remote, name), oid, snap, msgs);
    }
    let compare = Msg::RangeDiff(crate::range_diff::RangeDiffMsg::OpenRewrite(format!("refs/remotes/{full}")));
    menu_item(msgs, ui, t.tr("remote_branch.menu.range_diff"), compare);
    match fs.filter(|f| f.host.is_some()) {
        Some(fs) if fs.remote == remote => {
            ui.separator();
            match fs.pr_for_branch(name) {
                Some(pr) => {
                    let label = t.trf("pr.menu.open", &[("noun", &fs.pr_word(t)), ("pr", &fs.pr_label(pr.id))]);
                    menu_item(msgs, ui, label, Msg::OpenUrl(pr.url.clone()))
                }
                None => super::pr_menu_items::remote(ui, t, (name, oid), snap, fs, c.sel, msgs),
            }
        }
        // Another remote of a repository on a code host: pull requests go through the host's.
        Some(_) => {}
        None => {
            ui.separator();
            super::pr_menu_items::remote_no_host(ui, t, remote, name, snap, msgs);
        }
    }
    ui.separator();
    ref_visibility::menu_items(ui, t, &format!("refs/remotes/{full}"), filter, false, msgs);
    super::branch_colour_menu::items(ui, t, name, snap, msgs);
    ui.separator();
    ui.menu_button(t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("refs.copy.name"), Msg::Copy(full.clone()));
        let link = LinkTarget::RemoteBranch { remote: remote.to_owned(), name: name.to_owned() };
        menu_item(msgs, ui, t.tr("refs.copy.link"), Msg::DeepLink(DeepLinkMsg::Copy(link)));
    });
    ui.separator();
    menu_item(
        msgs,
        ui,
        t.tr("remote_branch.menu.delete"),
        guard(
            t,
            snap,
            name,
            Deleting::Remote(remote),
            Msg::confirm_danger(
                t.tr("remote_branch.delete.title"),
                t.trf("remote_branch.delete.body", &[("branch", &name), ("remote", &remote)]),
                t.tr("common.delete.button"),
                Msg::git("Delete remote branch", &["push", remote, "--delete", name]),
            ),
        ),
    );
}

/// Check out the tracking branch (or create one), create a branch here, and create a worktree.
fn checkout_items(
    ui: &mut egui::Ui,
    t: Lang,
    (remote, name): (&str, &str),
    oid: Oid,
    tracking: Option<&BranchInfo>,
    snap: &Snapshot,
    msgs: &mut Vec<Msg>,
) {
    match tracking {
        Some(b) if !b.is_head => {
            let local = b.name.as_str();
            let blocked = worktree_branches::checkout_blocked(snap, local);
            let label = t.trf("remote_branch.menu.checkout_local", &[("branch", &local)]);
            menu_item_if(msgs, ui, label, blocked.as_deref(), Msg::Branch(BranchMsg::Checkout(local.to_owned())));
        }
        Some(_) => {}
        None => {
            let blocked = remote_branch::checkout_blocked(snap, remote, name);
            let checkout = Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.to_owned(), name: name.to_owned() });
            menu_item_if(msgs, ui, t.tr("branch.menu.checkout"), blocked.as_deref(), checkout);
        }
    }
    let new_branch = Msg::Branch(BranchMsg::NewBranch { start: Some(oid.to_string()), name: String::new() });
    menu_item(msgs, ui, t.tr("commits.menu.create_branch"), new_branch);
    if name != "HEAD" {
        let base = WorktreeBase::Remote { remote: remote.to_owned(), branch: name.to_owned() };
        menu_item(msgs, ui, t.tr("branch.menu.create_worktree"), Msg::Worktree(WorktreeMsg::CreateDialog(base)));
    }
}

/// Operations against the checked-out branch `current`: merge, rebase, conflict check, compare
/// and set upstream.
fn current_items(ui: &mut egui::Ui, c: &RemoteMenu, (remote, name): (&str, &str), oid: Oid, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let (t, current) = (c.t, c.current);
    let args: [(&str, &dyn std::fmt::Display); 1] = [("branch", &current)];
    let full = format!("{remote}/{name}");
    menu_item(msgs, ui, t.trf("branch.menu.merge_into", &args), Msg::git("Merge", &["merge", &full]));
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Merge { theirs: full.clone(), oid }));
    menu_item(msgs, ui, t.tr("op_preview.menu.merge"), preview);
    menu_item(msgs, ui, t.trf("branch.menu.rebase_onto", &args), Msg::git("Rebase", &["rebase", &full]));
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Rebase { onto: full.clone(), oid }));
    menu_item(msgs, ui, t.tr("op_preview.menu.rebase"), preview);
    let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
    let blocked = onto_commit_blocker(t, snap, oid);
    menu_item_if(msgs, ui, t.trf("branch.menu.interactive_onto", &args), blocked.as_deref(), onto);
    let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(full.clone()));
    menu_item(msgs, ui, t.trf("branch.menu.check_conflicts_with", &args), check);
    let compare = Msg::Selection(SelectionMsg::CompareWithHead { oid, name: full.clone() });
    let blocked = remote_branch::compare_blocked(snap, oid, &full);
    menu_item_if(msgs, ui, t.trf("remote_branch.menu.compare_with", &args), blocked.as_deref(), compare);
    if !remote_branch::is_upstream_of_head(snap, remote, name) {
        let set = BranchMsg::SetUpstream { branch: current.to_owned(), remote: remote.to_owned(), name: name.to_owned() };
        menu_item(msgs, ui, t.trf("remote_branch.menu.set_as_upstream", &args), Msg::Branch(set));
    }
}

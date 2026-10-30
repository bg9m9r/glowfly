//! The right-click menu of a remote-tracking branch, shared by the REMOTE rows of the left panel
//! and the remote-branch labels in the graph, in groups: checkout (the local branch tracking it,
//! or a new one), a new branch and a worktree; fast-forward of that local branch, merge, rebase,
//! conflict check, compare and set upstream against the checked-out branch, and comparing it
//! with its version before the last force-push (Range-diff), merge and rebase as split rows
//! whose eye button previews them; pull requests (also from a fork remote into the base
//! repository); hide or solo, and its colour; copying its name or a link; and delete on the
//! remote (its dialog, `remote_branch_delete_dialog.rs`, warns of local branches tracking it).
//! The branch's name heads the menu once; the items leave it out, and other names in
//! labels are cut to fit, in full on hover. Disabled items say why on hover, with the same text
//! `update` logs when it refuses a late message.

use super::common::{menu_danger, menu_header, menu_item, menu_item_if, menu_row, menu_row_preview};
use super::ref_visibility;
use crate::conflict_check::ConflictCheckMsg;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::{BranchMsg, ForgeMsg, Msg, OpPreviewMsg, RebaseMsg, SelectionMsg};
use crate::state::rebase_onto::onto_commit_blocker;
use crate::state::{ForgeState, Selection, branches, pr_repos, remote_branch, worktree_branches};
use crate::ui::widgets::menu::{item_named, named, separator};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::settings::GraphFilter;
use glowfly_core::worktree_ops::WorktreeBase;

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
    separator(ui);
    if let Some(b) = tracking {
        let blocked = branches::fast_forward_blocked(b, snap.state);
        let ff = Msg::Branch(BranchMsg::FastForwardBranch(b.name.clone()));
        let label = named(t.tr("remote_branch.menu.fast_forward_local"), &[("branch", &b.name)]);
        menu_row(msgs, ui, item_named(label).blocked(blocked.as_deref()), ff);
    }
    if !current.is_empty() {
        current_items(ui, c, (remote, name), oid, snap, msgs);
    }
    let compare = Msg::RangeDiff(crate::range_diff::RangeDiffMsg::OpenRewrite(format!("refs/remotes/{full}")));
    menu_item(msgs, ui, t.tr("remote_branch.menu.range_diff"), compare);
    match fs.filter(|f| f.host.is_some()) {
        Some(fs) if fs.remote == remote => {
            separator(ui);
            match fs.pr_for_branch(name) {
                Some(pr) => {
                    let label = t.trf("pr.menu.open", &[("noun", &fs.pr_word(t)), ("pr", &fs.pr_label(pr.id))]);
                    menu_item(msgs, ui, label, Msg::OpenUrl(pr.url.clone()))
                }
                None => super::pr_menu_items::remote(ui, t, (name, oid), snap, fs, c.sel, msgs),
            }
        }
        // A fork on the code host's account: its listed pull request (matched by repository and
        // branch), else one from it into the base repository.
        Some(fs) if pr_repos::is_pr_remote(fs, remote) => {
            separator(ui);
            match fs.pr_from(remote, name) {
                Some(pr) => {
                    let label = t.trf("pr.menu.open", &[("noun", &fs.pr_word(t)), ("pr", &fs.pr_label(pr.id))]);
                    menu_item(msgs, ui, label, Msg::OpenUrl(pr.url.clone()))
                }
                None => {
                    let create = Msg::Forge(ForgeMsg::CreatePrFrom { remote: remote.to_owned(), source: name.to_owned() });
                    menu_item(msgs, ui, t.trf("pr.menu.create", &[("noun", &fs.pr_word(t))]), create);
                }
            }
        }
        // Another remote of a repository on a code host: pull requests go through the host's.
        Some(_) => {}
        None => {
            separator(ui);
            super::pr_menu_items::remote_no_host(ui, t, remote, name, snap, msgs);
        }
    }
    separator(ui);
    ref_visibility::menu_items(ui, t, &format!("refs/remotes/{full}"), filter, false, msgs);
    super::branch_colour_menu::items(ui, t, name, snap, msgs);
    separator(ui);
    menu_item(msgs, ui, t.tr("refs.copy.name_item"), Msg::Copy(full.clone()));
    let link = LinkTarget::RemoteBranch { remote: remote.to_owned(), name: name.to_owned() };
    menu_item(msgs, ui, t.tr("refs.copy.link_item"), Msg::DeepLink(DeepLinkMsg::Copy(link)));
    separator(ui);
    let delete = Msg::Branch(BranchMsg::DeleteRemoteDialog { remote: remote.to_owned(), name: name.to_owned() });
    menu_danger(msgs, ui, t.tr("remote_branch.menu.delete"), None, delete);
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
            let label = named(t.tr("remote_branch.menu.checkout_local"), &[("branch", local)]);
            menu_row(msgs, ui, item_named(label).blocked(blocked.as_deref()), Msg::Branch(BranchMsg::Checkout(local.to_owned())));
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

/// Operations against the checked-out branch `current`: merge and rebase (each with its
/// preview), interactive rebase, conflict check, compare and set upstream.
fn current_items(ui: &mut egui::Ui, c: &RemoteMenu, (remote, name): (&str, &str), oid: Oid, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let (t, current) = (c.t, c.current);
    let on = [("branch", current)];
    let full = format!("{remote}/{name}");
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Merge { theirs: full.clone(), oid }));
    let merge = item_named(named(t.tr("branch.menu.merge_into"), &on));
    menu_row_preview(msgs, ui, merge, (t.tr("op_preview.menu.merge"), None), Msg::git("Merge", &["merge", &full]), preview);
    let preview = Msg::OpPreview(OpPreviewMsg::Open(PreviewOp::Rebase { onto: full.clone(), oid }));
    let rebase = item_named(named(t.tr("branch.menu.rebase_onto"), &on));
    menu_row_preview(msgs, ui, rebase, (t.tr("op_preview.menu.rebase"), None), Msg::git("Rebase", &["rebase", &full]), preview);
    let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(oid)));
    let blocked = onto_commit_blocker(t, snap, oid);
    menu_row(msgs, ui, item_named(named(t.tr("branch.menu.interactive_onto"), &on)).blocked(blocked.as_deref()), onto);
    let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(full.clone()));
    menu_row(msgs, ui, item_named(named(t.tr("branch.menu.check_conflicts_with"), &on)), check);
    let compare = Msg::Selection(SelectionMsg::CompareWithHead { oid, name: full.clone() });
    let blocked = remote_branch::compare_blocked(snap, oid, &full);
    menu_row(msgs, ui, item_named(named(t.tr("remote_branch.menu.compare_with"), &on)).blocked(blocked.as_deref()), compare);
    if !remote_branch::is_upstream_of_head(snap, remote, name) {
        let set = BranchMsg::SetUpstream { branch: current.to_owned(), remote: remote.to_owned(), name: name.to_owned() };
        menu_row(msgs, ui, item_named(named(t.tr("remote_branch.menu.set_as_upstream"), &on)), Msg::Branch(set));
    }
}

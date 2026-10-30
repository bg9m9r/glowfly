//! Branch operations from the branch menus: checkout (local, or from a remote branch through
//! its tracking local branch, in `branch_checkout.rs`), rename (also
//! on the remote, and deleting a remote branch, in `branch_sync.rs`), set
//! and unset the upstream, fast-forward without checkout, push to the upstream or a chosen
//! remote (force push and push to all remotes in `branch_push.rs`), and delete together with
//! the remote branch (deleting the local branch alone is in
//! `branch_delete.rs`). Each re-checks the branch against the
//! current snapshot (a menu or confirm can be stale), then runs `branch_ops` commands as the
//! single active git job.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::BranchMsg;
use crate::state::branches::{self, NO_REMOTES};
use crate::state::worktree_branches;
use crate::state::{AppState, Modal};
use glowfly_core::branch_ops::{self, check_branch_name, is_valid_branch_name};
use glowfly_core::repo::BranchInfo;
use glowfly_core::undo::UndoKind;

pub(super) fn handle(st: &mut AppState, msg: BranchMsg, now: std::time::Instant, fx: &mut Vec<Effect>) {
    match msg {
        BranchMsg::Checkout(name) => super::branch_checkout::checkout(st, &name, fx),
        BranchMsg::CheckoutRemote { remote, name } => super::branch_checkout::checkout_remote(st, &remote, &name, fx),
        BranchMsg::CheckoutTracking { remote, name, local } => super::branch_checkout::checkout_tracking(st, &remote, &name, &local, fx),
        BranchMsg::CheckoutPullRequest { id } => super::pr_checkout::checkout(st, id, fx),
        BranchMsg::NewBranch { start, name } => {
            st.modal = Some(Modal::NewBranch { start, name, checkout: true, track: None, touched: Default::default() })
        }
        BranchMsg::NewTrackingBranch { remote, name } => super::branch_checkout::new_tracking(st, remote, name),
        BranchMsg::RenameBranchDialog(name) => {
            if let Some(b) = find(st, &name) {
                st.modal = Some(Modal::RenameBranch(crate::state::branch_sync::RenameForm::new(b.name)));
            }
        }
        BranchMsg::RenameBranch { old, new, remote } => rename(st, &old, new.trim(), remote, fx),
        BranchMsg::SetUpstreamDialog(name) => upstream_dialog(st, &name),
        BranchMsg::SetUpstream { branch, remote, name } => set_upstream(st, &branch, &remote, name.trim(), fx),
        BranchMsg::UnsetUpstream(name) => {
            let Some(b) = find(st, &name) else { return };
            if b.tracking.is_none() {
                st.log(false, format!("{name} has no upstream."));
                return;
            }
            start_job(st, JobSpec::new("Unset upstream".into(), vec![branch_ops::unset_upstream_cmd(&name)]), fx);
        }
        BranchMsg::FastForwardBranch(name) => fast_forward(st, &name, fx),
        BranchMsg::DeleteDialog(name) => super::branch_delete::open(st, &name, fx),
        BranchMsg::UnmergedCounted { name, tip, count } => super::branch_delete::counted(st, &name, tip, count),
        BranchMsg::Delete { name, tip, force } => super::branch_delete::delete(st, &name, tip, force, fx),
        BranchMsg::DeleteRemoteDialog { remote, name } => super::branch_sync::delete_remote_dialog(st, &remote, &name),
        BranchMsg::DeleteRemote { remote, name, unset } => super::branch_sync::delete_remote(st, &remote, &name, &unset, fx),
        BranchMsg::DeleteBranchAndRemote(name) => delete_with_remote(st, &name, fx),
        BranchMsg::PushBranch { branch, remote } => push(st, &branch, remote.as_deref(), fx),
        BranchMsg::ForcePush { branch, remote, dest, seen } => super::branch_push::force_push(st, &branch, (&remote, &dest), seen, fx),
        BranchMsg::PushAll(branch) => super::branch_push::push_all(st, &branch, fx),
        BranchMsg::PushedAll { branch, total, result } => super::branch_push::pushed_all(st, &branch, total, result, now, fx),
    }
}

/// Fast-forwards the local branch `name` to its upstream, unless that is refused.
pub(super) fn fast_forward(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, name) else { return };
    let state = st.snap.as_ref().map(|s| s.state).unwrap_or(git2::RepositoryState::Clean);
    if let Some(why) = branches::fast_forward_blocked(&b, state) {
        st.log(false, why);
        return;
    }
    if let Some(cmd) = branch_ops::fast_forward_cmd(&b) {
        start_job(st, JobSpec::new("Fast-forward".into(), vec![cmd]), fx);
    }
}

/// The local branch `name` in the current snapshot; logs when it is gone.
pub(super) fn find(st: &mut AppState, name: &str) -> Option<BranchInfo> {
    let snap = st.snap.as_ref()?;
    let b = snap.locals.iter().find(|b| b.name == name).cloned();
    if b.is_none() {
        st.log(false, format!("Branch {name} no longer exists."));
    }
    b
}

/// Renames `old` to `new`, and with `remote` its upstream too (update/branch_sync.rs).
fn rename(st: &mut AppState, old: &str, new: &str, remote: bool, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, old) else { return };
    if new == old {
        st.log(false, format!("{old} already has that name."));
        return;
    }
    let existing: Vec<String> = st.snap.as_ref().map(|s| s.locals.iter().map(|b| b.name.clone()).collect()).unwrap_or_default();
    let names: Vec<&str> = existing.iter().map(String::as_str).collect();
    if let Err(p) = check_branch_name(new, &names, Some(old)) {
        st.log(false, p.message(new));
        return;
    }
    if remote {
        super::branch_sync::rename_with_remote(st, &b, new, fx);
        return;
    }
    if matches!(st.modal, Some(Modal::RenameBranch(_))) {
        st.modal = None;
    }
    start_job(st, JobSpec::new("Rename branch".into(), vec![branch_ops::rename_cmd(old, new)]), fx);
}

fn upstream_dialog(st: &mut AppState, name: &str) {
    let Some(b) = find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    if snap.remotes.is_empty() {
        st.log(false, NO_REMOTES);
        return;
    }
    let tracked = branches::remote_tracking(&b);
    let remote = tracked.map(|t| t.remote.clone()).or_else(|| branches::default_remote(snap)).unwrap_or_default();
    let remote_branch = tracked.map(|t| t.merge.clone()).unwrap_or_else(|| b.name.clone());
    st.modal = Some(Modal::SetUpstream { branch: b.name, remote, name: remote_branch, touched: Default::default() });
}

fn set_upstream(st: &mut AppState, branch: &str, remote: &str, name: &str, fx: &mut Vec<Effect>) {
    if find(st, branch).is_none() {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let Some((_, branches)) = snap.remotes.iter().find(|(r, _)| r == remote) else {
        st.log(false, format!("There is no remote named {remote}."));
        return;
    };
    if !is_valid_branch_name(name) {
        st.log(false, "Not a valid branch name.");
        return;
    }
    let exists = branches.iter().any(|(n, _)| n == name);
    if matches!(st.modal, Some(Modal::SetUpstream { .. })) {
        st.modal = None;
    }
    start_job(st, JobSpec::new("Set upstream".into(), branch_ops::set_upstream_cmds(branch, remote, name, exists)), fx);
}

fn delete_with_remote(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, name) else { return };
    if b.is_head {
        st.log(false, format!("{name} is checked out. Check out another branch before deleting it."));
        return;
    }
    if let Some(why) = st.snap.as_ref().and_then(|s| worktree_branches::delete_blocked(s, name)) {
        st.log(false, why);
        return;
    }
    let Some(t) = branches::can_delete_with_remote(&b) else {
        st.log(false, format!("{name} has no upstream branch on a remote to delete."));
        return;
    };
    let head = st.snap.as_ref().and_then(|s| s.head_branch.as_deref());
    if let Some(why) = branches::delete_with_remote_blocked(&b, t, head) {
        st.log(false, why);
        return;
    }
    let cmds = branch_ops::delete_with_remote_cmds(name, t);
    let spec =
        JobSpec { record: Some(UndoKind::DeleteBranch(name.to_owned())), ..JobSpec::new("Delete local and remote branch".into(), cmds) };
    start_job(st, spec, fx);
}

fn push(st: &mut AppState, name: &str, remote: Option<&str>, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    // The state against the upstream is known only for its own remote.
    let to_upstream = remote.is_none() || branches::remote_tracking(&b).is_some_and(|t| Some(t.remote.as_str()) == remote);
    let blocked = if to_upstream {
        branches::push_blocked(&b, !snap.remotes.is_empty())
    } else {
        snap.remotes.is_empty().then(|| NO_REMOTES.to_owned())
    };
    if let Some(why) = blocked {
        st.log(false, why);
        return;
    }
    let Some((target_remote, target, set_upstream)) = branches::push_target(&b, snap, remote) else {
        st.log(false, format!("There is no remote named {}.", remote.unwrap_or_default()));
        return;
    };
    let label = match remote {
        Some(r) => format!("Push to {r}"),
        None => "Push".to_owned(),
    };
    let cmd = branch_ops::push_cmd(name, &target_remote, &target, set_upstream);
    start_job(st, JobSpec::new(label, vec![cmd]), fx);
}

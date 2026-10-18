//! Branch operations from the branch menu: checkout (refused for a branch another worktree
//! has), rename, set and unset the upstream, fast-forward without checkout, push to the upstream
//! or a chosen remote, and delete together with the remote branch. Each re-checks the branch against the current snapshot (a menu or confirm can
//! be stale), then runs `branch_ops` commands as the single active git job.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::BranchMsg;
use crate::state::branches::{self, NO_REMOTES};
use crate::state::worktree_branches;
use crate::state::{AppState, Modal};
use trellis_core::branch_ops::{self, check_branch_name, is_valid_branch_name};
use trellis_core::repo::BranchInfo;
use trellis_core::undo::UndoKind;

pub(super) fn handle(st: &mut AppState, msg: BranchMsg, fx: &mut Vec<Effect>) {
    match msg {
        BranchMsg::Checkout(name) => checkout(st, &name, fx),
        BranchMsg::NewBranch { start, name } => st.modal = Some(Modal::NewBranch { start, name, checkout: true }),
        BranchMsg::RenameBranchDialog(name) => {
            if let Some(b) = find(st, &name) {
                st.modal = Some(Modal::RenameBranch { old: b.name.clone(), name: b.name });
            }
        }
        BranchMsg::RenameBranch { old, new } => rename(st, &old, new.trim(), fx),
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
        BranchMsg::FastForwardBranch(name) => {
            let Some(b) = find(st, &name) else { return };
            let state = st.snap.as_ref().map(|s| s.state).unwrap_or(git2::RepositoryState::Clean);
            if let Some(why) = branches::fast_forward_blocked(&b, state) {
                st.log(false, why);
                return;
            }
            if let Some(cmd) = branch_ops::fast_forward_cmd(&b) {
                start_job(st, JobSpec::new("Fast-forward".into(), vec![cmd]), fx);
            }
        }
        BranchMsg::DeleteBranchAndRemote(name) => delete_with_remote(st, &name, fx),
        BranchMsg::PushBranch { branch, remote } => push(st, &branch, remote.as_deref(), fx),
    }
}

/// The local branch `name` in the current snapshot; logs when it is gone.
fn find(st: &mut AppState, name: &str) -> Option<BranchInfo> {
    let snap = st.snap.as_ref()?;
    let b = snap.locals.iter().find(|b| b.name == name).cloned();
    if b.is_none() {
        st.log(false, format!("Branch {name} no longer exists."));
    }
    b
}

/// Checks out local branch `name`, unless it is HEAD or another worktree has it.
fn checkout(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, name) else { return };
    if b.is_head {
        st.log(false, format!("{name} is already checked out."));
        return;
    }
    if let Some(why) = st.snap.as_ref().and_then(|s| worktree_branches::checkout_blocked(s, name)) {
        st.log(false, why);
        return;
    }
    // A single command: `start_job` records it for undo as `JobMsg::Git` would.
    start_job(st, JobSpec::new("Checkout".into(), vec![branches::checkout_local(name)]), fx);
}

fn rename(st: &mut AppState, old: &str, new: &str, fx: &mut Vec<Effect>) {
    if find(st, old).is_none() {
        return;
    }
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
    if matches!(st.modal, Some(Modal::RenameBranch { .. })) {
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
    st.modal = Some(Modal::SetUpstream { branch: b.name, remote, name: remote_branch });
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

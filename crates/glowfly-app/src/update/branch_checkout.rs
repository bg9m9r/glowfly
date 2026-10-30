//! Checking out a branch from the branch menus: a local branch (refused for one another worktree
//! has), and a remote branch through a local branch tracking it (`BranchMsg::CheckoutRemote`,
//! or one chosen tracker with `CheckoutTracking`), or a new one tracking it. A tracker that is
//! only behind is fast-forwarded in the same job, strictly (`branch_ops::fast_forward_ref_cmds`);
//! a diverged one is checked out where it is, with a note saying how far each side has gone.
//! Every checkout goes through the auto-stash guard.

use super::ops::JobSpec;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::remote_branch::{self, TrackingCheckout};
use crate::state::{AppState, branches, worktree_branches};
use glowfly_core::branch_ops;
use glowfly_core::stash_ops::AutoStashFor;
use glowfly_core::undo::UndoKind;

/// Checks out local branch `name`, unless it is HEAD or another worktree has it.
pub(super) fn checkout(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    if b.is_head {
        st.log(false, format!("{name} is already checked out."));
        return;
    }
    if let Some(why) = st.snap.as_ref().and_then(|s| worktree_branches::checkout_blocked(s, name)) {
        st.log(false, why);
        return;
    }
    // A single command: `start_job` records it for undo as `JobMsg::Git` would.
    let spec = JobSpec::new("Checkout".into(), vec![branches::checkout_local(name)]);
    let retry = || Msg::Branch(BranchMsg::Checkout(name.to_owned()));
    super::auto_stash::start_guarded(st, spec, retry, fx);
}

/// Checks out the local branch tracking `remote`/`name` (`tracking_local`), or creates one
/// tracking it, unless the remote branch is gone or a local branch already has its name.
pub(super) fn checkout_remote(st: &mut AppState, remote: &str, name: &str, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if remote_branch::exists(snap, remote, name).is_none() {
        let t = st.settings.lang();
        st.log(false, t.trf("remote_branch.checkout.gone", &[("upstream", &format!("{remote}/{name}"))]));
        return;
    }
    if let Some(local) = remote_branch::tracking_local(snap, remote, name).map(|b| b.name.clone()) {
        checkout_tracking(st, remote, name, &local, fx);
        return;
    }
    if let Some(why) = remote_branch::checkout_blocked(snap, remote, name) {
        st.log(false, why);
        return;
    }
    // A single command: `start_job` records it for undo (`UndoKind::Checkout`).
    let spec = JobSpec::new("Checkout".into(), vec![remote_branch::checkout_cmd(remote, name)]);
    let retry = || Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.to_owned(), name: name.to_owned() });
    super::auto_stash::start_guarded(st, spec, retry, fx);
}

/// Checks out `local`, which tracks `remote`/`name`, fast-forwarding it to that branch first
/// when it is only behind. The checked-out one is fast-forwarded in place (as
/// `FastForwardBranch` does, update/branches.rs); one another worktree has is refused before
/// anything moves.
pub(super) fn checkout_tracking(st: &mut AppState, remote: &str, name: &str, local: &str, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let upstream = format!("{remote}/{name}");
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(tip) = remote_branch::exists(snap, remote, name) else {
        st.log(false, t.trf("remote_branch.checkout.gone", &[("upstream", &upstream)]));
        return;
    };
    let Some(b) = snap.locals.iter().find(|b| b.name == local && remote_branch::tracks(b, remote, name)).cloned() else {
        st.log(false, t.trf("remote_branch.checkout.not_tracking", &[("branch", &local), ("upstream", &upstream)]));
        return;
    };
    let plan = remote_branch::tracking_checkout(&b, tip);
    if b.is_head {
        match plan {
            TrackingCheckout::FastForward { .. } => super::branches::fast_forward(st, &b.name, fx),
            _ => st.log(false, t.trf("remote_branch.checkout.already", &[("branch", &local)])),
        }
        return;
    }
    if let Some(why) = worktree_branches::checkout_blocked(snap, local) {
        st.log(false, why);
        return;
    }
    match plan {
        TrackingCheckout::Plain => checkout(st, local, fx),
        TrackingCheckout::Diverged { ahead, behind } => {
            st.log(true, remote_branch::diverged_note(t, local, &upstream, ahead, behind));
            checkout(st, local, fx);
        }
        TrackingCheckout::FastForward { from, to } => {
            let mut cmds = branch_ops::fast_forward_ref_cmds(local, from, to, &upstream);
            cmds.push(branches::checkout_local(local));
            let spec = JobSpec { record: Some(UndoKind::Checkout), ..JobSpec::new("Checkout".into(), cmds) };
            let op = AutoStashFor::Checkout { target: local.to_owned() };
            let retry =
                || Msg::Branch(BranchMsg::CheckoutTracking { remote: remote.to_owned(), name: name.to_owned(), local: local.to_owned() });
            super::auto_stash::start_guarded_as(st, spec, op, retry, fx);
        }
    }
}

/// Opens the New branch dialog at `remote`/`name`, for a branch tracking it under a name of its
/// own, unless the remote branch is gone.
pub(super) fn new_tracking(st: &mut AppState, remote: String, name: String) {
    let Some(tip) = st.snap.as_ref().and_then(|s| remote_branch::exists(s, &remote, &name)) else {
        let t = st.settings.lang();
        st.log(false, t.trf("remote_branch.checkout.gone", &[("upstream", &format!("{remote}/{name}"))]));
        return;
    };
    let start = Some(tip.to_string());
    st.modal = Some(crate::state::Modal::NewBranch {
        start,
        name: String::new(),
        checkout: true,
        track: Some((remote, name)),
        touched: Default::default(),
    });
}

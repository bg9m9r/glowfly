//! Checking out a branch from the branch menus: a local branch (opening the worktree that has it
//! instead, update/branch_elsewhere.rs), and a remote branch through a local branch tracking it (`BranchMsg::CheckoutRemote`,
//! or one chosen tracker with `CheckoutTracking`), or a new one tracking it. A tracker that is
//! only behind is fast-forwarded in the same job, strictly (`branch_ops::fast_forward_ref_cmds`);
//! a diverged one is checked out where it is, with a note saying how far each side has gone.
//! Every checkout goes through the auto-stash guard.

use super::branch_elsewhere::{Go, Route, route, with_prune};
use super::ops::JobSpec;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::remote_branch::{self, TrackingCheckout};
use crate::state::{AppState, branches};
use glowfly_core::branch_ops;
use glowfly_core::stash_ops::AutoStashFor;
use glowfly_core::undo::UndoKind;

/// Runs checkout `m` pruning first the missing worktree that has its branch (Prune and check out).
pub(super) fn prune_then(st: &mut AppState, m: BranchMsg, now: std::time::Instant, fx: &mut Vec<Effect>) {
    let go = Go { now, prune: true };
    match m {
        BranchMsg::Checkout(name) => checkout(st, &name, go, fx),
        BranchMsg::CheckoutRemote { remote, name } => checkout_remote(st, &remote, &name, go, fx),
        BranchMsg::CheckoutTracking { remote, name, local } => checkout_tracking(st, &remote, &name, &local, go, fx),
        BranchMsg::CheckoutPullRequest { id } => super::pr_checkout::checkout(st, id, go, fx),
        _ => {}
    }
}

/// Checks out local branch `name`, unless it is HEAD; opens the worktree that has it instead.
pub(super) fn checkout(st: &mut AppState, name: &str, go: Go, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    if b.is_head {
        st.log(false, format!("{name} is already checked out."));
        return;
    }
    let again = || BranchMsg::Checkout(name.to_owned());
    let how = route(st, name, go, again, fx);
    if how == Route::Done {
        return;
    }
    // A single command: `start_job` records it for undo as `JobMsg::Git` would; after a prune,
    // it is recorded as the checkout.
    let mut spec = JobSpec::new("Checkout".into(), with_prune(&how, vec![branches::checkout_local(name)]));
    if how == Route::PruneFirst {
        spec.record = Some(UndoKind::Checkout);
    }
    let retry = || Msg::Branch(if go.prune { BranchMsg::PruneThen(Box::new(again())) } else { again() });
    super::auto_stash::start_guarded(st, spec, retry, fx);
}

/// Checks out the local branch tracking `remote`/`name` (`tracking_local`), or creates one
/// tracking it, unless the remote branch is gone or a local branch already has its name.
pub(super) fn checkout_remote(st: &mut AppState, remote: &str, name: &str, go: Go, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if remote_branch::exists(snap, remote, name).is_none() {
        let t = st.settings.lang();
        st.log(false, t.trf("remote_branch.checkout.gone", &[("upstream", &format!("{remote}/{name}"))]));
        return;
    }
    if let Some(local) = remote_branch::tracking_local(snap, remote, name).map(|b| b.name.clone()) {
        checkout_tracking(st, remote, name, &local, go, fx);
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
/// `FastForwardBranch` does, update/branches.rs); for one another worktree has, that worktree
/// opens instead and nothing moves.
pub(super) fn checkout_tracking(st: &mut AppState, remote: &str, name: &str, local: &str, go: Go, fx: &mut Vec<Effect>) {
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
    let again = || BranchMsg::CheckoutTracking { remote: remote.to_owned(), name: name.to_owned(), local: local.to_owned() };
    let how = route(st, local, go, again, fx);
    if how == Route::Done {
        return;
    }
    match plan {
        // `checkout` prunes first again when asked.
        TrackingCheckout::Plain => checkout(st, local, go, fx),
        TrackingCheckout::Diverged { ahead, behind } => {
            st.log(true, remote_branch::diverged_note(t, local, &upstream, ahead, behind));
            checkout(st, local, go, fx);
        }
        TrackingCheckout::FastForward { from, to } => {
            let mut cmds = with_prune(&how, branch_ops::fast_forward_ref_cmds(local, from, to, &upstream));
            cmds.push(branches::checkout_local(local));
            let spec = JobSpec { record: Some(UndoKind::Checkout), ..JobSpec::new("Checkout".into(), cmds) };
            let op = AutoStashFor::Checkout { target: local.to_owned() };
            let retry = || Msg::Branch(if go.prune { BranchMsg::PruneThen(Box::new(again())) } else { again() });
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

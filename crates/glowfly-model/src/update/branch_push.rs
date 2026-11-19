//! Force push with lease of any local branch, from its row's menu or a rejected push's toast,
//! the toolbar's Force push of the checked-out branch, and Push to all remotes
//! (state/branches/push.rs). Where a force push goes is `force_dest`'s one rule. A row's or
//! toast's force push re-checks the refusals against the current snapshot (never the
//! destination's default branch or a Git Flow base; the toolbar's confirmation only warns about
//! those), refuses when its destination or that destination's tip changed since the
//! confirmation counted its commits, and runs `force_push_cmd` with that lease. Push to all
//! remotes runs one job (`BranchEffect::PushAll`) that pushes to each remote in turn, never
//! forced, and names the remotes that failed. Neither moves a local branch, so the undo history
//! stays.

use super::ops::{JobSpec, finish_job, start_job};
use crate::effect::{BranchEffect, Effect};
use crate::state::AppState;
use crate::state::branches::NO_REMOTES;
use crate::state::branches::force_dest;
use crate::state::branches::push::{changed, force_blocked, pushed_all_headline};
use crate::state::worktree_branches::held_blocked;
use git2::Oid;
use glowfly_core::push_all::{self, PushedAll};
use std::time::Instant;

/// The job label of Push to all remotes (English: it is also log text).
const PUSH_ALL: &str = "Push to all remotes";

/// Force pushes `name` to the upstream the confirmation named (`dest` on `remote`), leasing
/// the tip it saw (`seen`).
pub(super) fn force_push(st: &mut AppState, name: &str, (remote, dest): (&str, &str), seen: Option<Oid>, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    let t = st.settings.lang();
    let now = force_dest::of(t, snap, &b);
    // Held by another worktree, or a destination that is a default or base branch. A
    // destination that is gone or refused is told below, as what changed since the confirmation.
    let refused = if now.is_ok() { force_blocked(t, snap, &b) } else { held_blocked(t, snap, name) };
    if let Some(why) = refused {
        st.log(false, why);
        return;
    }
    // Its destination isn't the one the confirmation named.
    let d = match now {
        Ok(d) if d.is(remote, dest) => d,
        other => {
            st.log(false, changed(t, &b, (remote, dest), other));
            return;
        }
    };
    // A fetch since the confirmation moved it: its count no longer holds.
    if d.seen != seen {
        st.log(false, t.trf("branch_push.force.moved", &[("upstream", &d.label), ("branch", &name)]));
        return;
    }
    let if_includes = st.git.has(glowfly_core::git_version::Feature::ForceIfIncludes);
    let cmd = super::ops::force_push_cmd(name, &d, if_includes);
    start_job(st, JobSpec::new("Force push".into(), vec![cmd]), fx);
}

/// The toolbar's Force push of the checked-out branch (to its upstream, or as a new branch
/// with `-u`), with the lease the snapshot saw; with nowhere to push, the Publish dialog. A
/// default or base branch is allowed: its confirmation said so (`head_force_confirm`).
pub(super) fn force_head(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(b) = st.snap.as_ref().and_then(|s| s.head_info()).cloned() else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    let t = st.settings.lang();
    match force_dest::of_head(t, snap, &b) {
        Err(why) => st.log(false, why),
        // No remote to push to: offer to create the repository on GitHub.
        Ok(None) => super::publish::open(st, fx),
        Ok(Some(d)) => {
            let if_includes = st.git.has(glowfly_core::git_version::Feature::ForceIfIncludes);
            let cmd = super::ops::force_push_cmd(&b.name, &d, if_includes);
            start_job(st, JobSpec::new("Force push".into(), vec![cmd]), fx);
        }
    }
}

/// Pushes `name` to every remote as one job.
pub(super) fn push_all(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    if snap.remotes.is_empty() {
        st.log(false, NO_REMOTES);
        return;
    }
    let pushes = push_all::plan(&b, snap.remotes.iter().map(|(r, _)| r.as_str()));
    let branch = b.name;
    super::ops::begin_job(st, PUSH_ALL, true, move |cwd| Effect::Branch(BranchEffect::PushAll { cwd, branch, pushes }), fx);
}

/// Push to all remotes ended: its run and toast name the remotes that failed.
pub(super) fn pushed_all(st: &mut AppState, branch: &str, total: usize, result: PushedAll, now: Instant, fx: &mut Vec<Effect>) {
    let headline = pushed_all_headline(st.settings.lang(), branch, total, &result.failed);
    finish_job(st, result.failed.is_empty(), result.output, Some(headline), None, now, fx);
}

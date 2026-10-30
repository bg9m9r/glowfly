//! Force push with lease of any local branch with an upstream, from its row's menu or a
//! rejected push's toast, the toolbar's Force push of the checked-out branch, and Push to all
//! remotes (state/branches/push.rs). A row's or toast's force push re-checks the refusals
//! against the current snapshot (never the remote's default branch or a Git Flow base; the
//! toolbar's confirmation only warns about those), refuses when the upstream or its tip changed
//! since the confirmation counted its commits, and runs `force_push_cmd` with that lease. Push to all
//! remotes runs one job (`BranchEffect::PushAll`) that pushes to each remote in turn, never
//! forced, and names the remotes that failed. Neither moves a local branch, so the undo history
//! stays.

use super::ops::{JobSpec, finish_job, start_job};
use crate::effect::{BranchEffect, Effect};
use crate::state::AppState;
use crate::state::branches::NO_REMOTES;
use crate::state::branches::push::{force_blocked, pushed_all_headline, seen_tip};
use git2::Oid;
use glowfly_core::push_all::{self, PushedAll};
use std::time::Instant;

/// The job label of Push to all remotes (English: it is also log text).
pub const PUSH_ALL: &str = "Push to all remotes";

/// Force pushes `name` to the upstream the confirmation named (`dest` on `remote`), leasing
/// the tip it saw (`seen`).
pub(super) fn force_push(st: &mut AppState, name: &str, (remote, dest): (&str, &str), seen: Option<Oid>, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    let t = st.settings.lang();
    if let Some(why) = force_blocked(t, snap, &b) {
        st.log(false, why);
        return;
    }
    // The upstream was set to another branch since the confirmation.
    if crate::state::branches::remote_tracking(&b).is_none_or(|up| up.remote != remote || up.merge != dest) {
        st.log(false, t.trf("branch_push.force.upstream_changed", &[("branch", &name), ("upstream", &format!("{remote}/{dest}"))]));
        return;
    }
    // A fetch since the confirmation moved the upstream: its count no longer holds.
    if seen_tip(snap, &b) != Some(seen) {
        let upstream = crate::state::branches::upstream_label(&b).unwrap_or_default();
        st.log(false, t.trf("branch_push.force.moved", &[("upstream", &upstream), ("branch", &name)]));
        return;
    }
    let if_includes = st.git.has(glowfly_core::git_version::Feature::ForceIfIncludes);
    let Some(cmd) = super::ops::force_push_cmd(&b, snap, if_includes) else { return };
    start_job(st, JobSpec::new("Force push".into(), vec![cmd]), fx);
}

/// The toolbar's Force push of the checked-out branch (to its upstream, or as a new branch
/// with `-u`), with the lease the snapshot saw; with nowhere to push, the Publish dialog. A
/// default or base branch is allowed: its confirmation said so (`head_force_confirm`).
pub(super) fn force_head(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(b) = st.snap.as_ref().and_then(|s| s.head_info()).cloned() else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    let if_includes = st.git.has(glowfly_core::git_version::Feature::ForceIfIncludes);
    let Some(cmd) = super::ops::force_push_cmd(&b, snap, if_includes) else {
        // No remote to push to: offer to create the repository on GitHub.
        return super::publish::open(st, fx);
    };
    start_job(st, JobSpec::new("Force push".into(), vec![cmd]), fx);
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

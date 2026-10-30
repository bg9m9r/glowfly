//! Force push with lease of any local branch with an upstream, from its row's menu or a
//! rejected push's toast, and Push to all remotes (state/branch_push.rs). Force push re-checks
//! the refusals against the current snapshot, refuses when the upstream moved since the
//! confirmation counted its commits, and runs `force_push_cmd` with that lease. Push to all
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

/// Force pushes `name` to its upstream, leasing the tip the confirmation saw (`seen`).
pub(super) fn force_push(st: &mut AppState, name: &str, seen: Option<Oid>, fx: &mut Vec<Effect>) {
    let Some(b) = super::branches::find(st, name) else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    let t = st.settings.lang();
    if let Some(why) = force_blocked(t, snap, &b) {
        st.log(false, why);
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

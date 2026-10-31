//! Restacking stacked branches as the single active job, through the rebase flow: refusing
//! what can't run, asking about uncommitted changes, running the steps, and handing every
//! result to `rebase::finished` (the banner, a stop's headline, the toast). Uncommitted changes
//! are stashed once for the whole restack and put back by the restack itself on the branch
//! checked out before (update/auto_stash_restack.rs tells how that went). A restack stopped
//! on a conflict waits in `st.restacks` for the banner's Continue or Skip, which runs its
//! remaining steps, while its stash waits as a pending auto-stash (saved for a restart). A
//! rebase that ends any other way (Abort) drops the steps left at the next refresh and ends
//! the restack as a quiet job (`RebaseEffect::EndRestack`): back onto that branch, the stash
//! put back, the finished part recorded as one undo entry. A successful amend or rewrite of a
//! branch with branches stacked on it offers a restack.

use super::auto_stash_restack as stash;
use super::jobs::begin_background_job;
use super::ops::Started;
use super::rebase::refuse;
use crate::effect::{Effect, RebaseEffect, RestackJob, UndoResult};
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::ToastAction;
use crate::state::rebase::{STASH_LINE, needs_stash};
use crate::state::stack::{PendingRestack, blocker, detail, not_restacked, plan_for};
use git2::RepositoryState;
use glowfly_core::git_version::Feature;
use glowfly_core::paths::same_spelling;
use glowfly_core::rebase::{Outcome, Resume};
use glowfly_core::repo::Snapshot;
use glowfly_core::stack::{RestackRun, RestackScope, stash_op};
use glowfly_core::undo::{Action, MoveOp};
use std::time::Instant;

pub const JOB_LABEL: &str = "Restack";
/// The quiet job that ends a stopped restack whose rebase ended another way.
const END_LABEL: &str = "End restack";

/// Starts the restack `scope` asks for, unless it is refused or must ask about uncommitted
/// changes first (`autostash`: the question was asked and answered).
pub(super) fn restack(st: &mut AppState, scope: RestackScope, autostash: bool, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return };
    if let Some(why) = st.git.blocked(Feature::UpdateRefs).or_else(|| blocker(t, snap, &scope)) {
        refuse(st, why, now);
        return;
    }
    let stash = needs_stash(snap);
    // Always stash (Preferences > General) answers the question.
    let autostash = autostash || (stash && st.settings.auto_stash);
    if stash && !autostash {
        let then = Msg::Rebase(RebaseMsg::Restack { scope, autostash: true });
        let ask = Msg::confirm(t.tr("stack.stash.title"), STASH_LINE.into(), t.tr("stack.offer.button"), then);
        fx.extend(super::update_sealed(st, ask, now));
        return;
    }
    let plan = plan_for(snap, &scope, autostash);
    let stash_op = autostash.then(|| stash_op(&plan));
    let job = RestackJob { plan, before: None, detail: detail(snap, &scope), stopped: None, stash_op };
    let effect = move |cwd| Effect::Rebase(RebaseEffect::Restack { cwd, job });
    if super::ops::begin_job(st, JOB_LABEL, false, effect, fx) != Started::No {
        super::submodule_batch::job_started(st, JOB_LABEL, true);
    }
}

/// A restack job ended (its first run, or a banner's Continue or Skip): a stop keeps the steps
/// left for the banner, with the stash held until the restack ends (as does a run that couldn't
/// go on while its stash waits), and the result ends the job as a rebase's would; then how
/// putting the stash back went, when the restack tried.
pub(super) fn ran(
    st: &mut AppState,
    result: Result<RestackRun, String>,
    undo: Option<UndoResult>,
    job: RestackJob,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(workdir) = st.job.as_ref().and_then(|j| j.workdir.clone()) else { return };
    st.restacks.retain(|p| !same_spelling(&p.workdir, &workdir));
    let waits = match &result {
        Ok(run) => matches!(run.outcome, Outcome::Stopped { .. }) || run.left.stash.is_some(),
        Err(_) => job.plan.stash.is_some(),
    };
    let put_back = result.as_ref().ok().and_then(|r| Some((r.put_back.clone()?, output_of(&r.outcome).to_owned())));
    if let Ok(run) = &result {
        stayed_on(st, run, &job);
    }
    if waits {
        stash::waits(st, &workdir, &job, fx);
        st.restacks.push(PendingRestack { workdir: workdir.clone(), job: job.clone() });
    }
    super::rebase::finished(st, result.map(|r| r.outcome), undo, now, fx);
    if let (Some(((oid, outcome), output)), Some(op)) = (put_back, &job.stash_op) {
        stash::put_back(st, &workdir, op, oid, outcome, &output, now, fx);
    }
}

/// The quiet job ending a stopped restack is done (`RebaseEffect::EndRestack`): it ends with the
/// finished part recorded, and tells how putting the stash back went.
pub(super) fn ended(
    st: &mut AppState,
    result: Result<RestackRun, String>,
    undo: Option<UndoResult>,
    job: RestackJob,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(workdir) = st.job.as_ref().and_then(|j| j.workdir.clone()) else { return };
    let (ok, output) = match &result {
        Ok(run) => (true, output_of(&run.outcome).to_owned()),
        Err(e) => (false, e.clone()),
    };
    if let Ok(run) = &result {
        stayed_on(st, run, &job);
    }
    super::ops::finish_job(st, ok, output.clone(), None, undo, now, fx);
    let put_back = result.ok().and_then(|r| r.put_back);
    match (put_back, &job.stash_op) {
        (Some((oid, outcome)), Some(op)) => stash::put_back(st, &workdir, op, oid, outcome, &output, now, fx),
        // Not ended (still rebasing, or unreadable): the stash stays listed.
        (None, Some(op)) if job.plan.stash.is_some() && !ok => stash::release(st, &workdir, op, fx),
        _ => {}
    }
}

/// The log line when checking the branch checked out before out again was refused.
fn stayed_on(st: &mut AppState, run: &RestackRun, job: &RestackJob) {
    if let (Some(top), Some(branch)) = (&run.stayed_on, &job.plan.return_to) {
        let text = st.settings.lang().trf("stack.stayed_on", &[("top", top), ("branch", branch)]);
        st.log(false, text);
    }
}

fn output_of(outcome: &Outcome) -> &str {
    match outcome {
        Outcome::Finished { output, .. } | Outcome::Stopped { output, .. } | Outcome::Failed { output } => output,
    }
}

/// The stopped restack of the active tab's repository, which the banner's Continue or Skip goes
/// on with.
pub(super) fn resume_job(st: &AppState) -> Option<RestackJob> {
    let here = st.workdir()?;
    st.restacks.iter().find(|p| same_spelling(&p.workdir, &here)).map(|p| p.job.clone())
}

/// Starts the banner's Continue or Skip of a stopped restack, when the active tab has one.
/// Returns false when it has none (an ordinary rebase goes on instead).
pub(super) fn resume(st: &mut AppState, how: Resume, fx: &mut Vec<Effect>) -> bool {
    let Some(job) = resume_job(st) else { return false };
    let effect = move |cwd| Effect::Rebase(RebaseEffect::ResumeRestack { cwd, how, job });
    if super::ops::begin_job(st, JOB_LABEL, false, effect, fx) != Started::No {
        super::submodule_batch::job_started(st, JOB_LABEL, true);
    }
    true
}

/// After a refresh: a stopped restack whose rebase ended some other way (Abort, or outside
/// Glowfly) is dropped, naming the branches it didn't get to, and ended by a quiet job. When
/// that job can't start, its stash stays in the stash list (said in the log).
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot, fx: &mut Vec<Effect>) {
    use RepositoryState::*;
    if st.job.is_some() || matches!(snap.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase) {
        return;
    }
    let Some(i) = st.restacks.iter().position(|p| same_spelling(&p.workdir, &snap.workdir)) else { return };
    let pending = st.restacks.remove(i);
    let tops = not_restacked(snap, &pending.job);
    if !tops.is_empty() {
        let text = st.settings.lang().trf("stack.dropped", &[("branches", &tops.join(", "))]);
        st.log(false, text);
    }
    let (job, workdir) = (pending.job, pending.workdir);
    let op = job.stash_op.clone().filter(|_| job.plan.stash.is_some());
    let effect = move |cwd| Effect::Rebase(RebaseEffect::EndRestack { cwd, job });
    if begin_background_job(st, END_LABEL, effect, fx) == Started::No
        && let Some(op) = op
    {
        stash::release(st, &workdir, &op, fx);
    }
}

/// After a job in the active tab (`here`) succeeded: an amend or history rewrite of a branch
/// that, in the snapshot from before the job, had branches stacked on it offers to restack
/// them.
pub(super) fn job_finished(st: &mut AppState, ok: bool, here: bool, undo: &Option<UndoResult>, now: Instant) {
    if !ok || !here {
        return;
    }
    let branch = match undo {
        Some(UndoResult::Recorded(Some(
            Action::Commit { amend: true, branch: Some(b), .. }
            | Action::Moved { op: MoveOp::Rewrite | MoveOp::Rebase, branch: Some(b), .. },
        ))) => b.clone(),
        _ => return,
    };
    if st.snap.as_ref().is_none_or(|s| s.stacks.above(&branch).is_empty()) {
        return;
    }
    let t = st.settings.lang();
    let msg = Msg::Rebase(RebaseMsg::Restack { scope: RestackScope::Above(branch.clone()), autostash: false });
    let action = ToastAction { label: t.tr("stack.offer.button").into(), msg: Box::new(msg), workdir: st.workdir() };
    st.toast_action(true, t.trf("stack.offer.text", &[("branch", &branch)]), action, now);
}

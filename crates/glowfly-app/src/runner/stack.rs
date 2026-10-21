//! Stack jobs: running a restack's steps with `glowfly_core::stack` (inside a journal note,
//! like other history rewrites) and going on with a stopped one after the banner's Continue or
//! Skip, each recording what the run means for the undo history; and pushing a stack's layers
//! (`Effect::Stack`), which moves no local branch, so it needs no note and no undo record.

use super::err;
use crate::effect::{RestackJob, StackEffect, UndoResult};
use crate::msg::{Msg, RebaseMsg, StackMsg};
use glowfly_core::error::Result;
use glowfly_core::journal::Op;
use glowfly_core::rebase::{Outcome, Resume};
use glowfly_core::stack::{self, RestackPlan, RestackRun};
use glowfly_core::undo::{self, RestackBefore};
use std::path::{Path, PathBuf};

pub(super) fn run(e: StackEffect) -> Msg {
    match e {
        StackEffect::Push { cwd, job } => {
            let result = stack::push(&cwd, &job.targets).map_err(err);
            Msg::Stack(StackMsg::Pushed { result, job: Box::new(job) })
        }
    }
}

pub(super) fn restack(cwd: PathBuf, job: RestackJob) -> Msg {
    let before = job.before.clone().or_else(|| undo::restack_before(&cwd));
    let result = super::journal::around(&cwd, Some(Op::Rewrite), || stack::restack(&cwd, &job.plan));
    ran(&cwd, before, job, result)
}

pub(super) fn resume(cwd: PathBuf, how: Resume, job: RestackJob) -> Msg {
    let before = job.before.clone().or_else(|| undo::restack_before(&cwd));
    let result = super::journal::around(&cwd, Some(Op::Rewrite), || stack::resume(&cwd, how, &job.plan));
    ran(&cwd, before, job, result)
}

/// The result message: the job carries the steps still left, the tips before its first run and,
/// after a stop, the step it stopped in (the one before the steps left, or the same one again
/// when Continue or Skip stopped in it once more).
fn ran(cwd: &Path, before: Option<RestackBefore>, job: RestackJob, result: Result<RestackRun>) -> Msg {
    let undo = recorded(cwd, before.as_ref(), &job.detail, &result);
    let stopped = match &result {
        Ok(r) if matches!(r.outcome, Outcome::Stopped { .. }) => {
            let i = job.plan.steps.len().checked_sub(r.left.steps.len() + 1);
            i.and_then(|i| job.plan.steps.get(i).cloned()).or(job.stopped)
        }
        _ => None,
    };
    let plan = result.as_ref().map(|r| r.left.clone()).unwrap_or_else(|_| RestackPlan::default());
    let job = RestackJob { plan, before, detail: job.detail, stopped };
    Msg::Rebase(RebaseMsg::RestackRan { result: result.map_err(err), undo, job: Box::new(job) })
}

/// What a restack run means for the undo history: a finished one is one action moving every
/// branch back (`None`, clearing the history, when the tips before are unknown); a stopped
/// one, or one that failed without moving a branch, keeps the history; one that failed after
/// moving some (`None`) clears it.
fn recorded(cwd: &Path, before: Option<&RestackBefore>, detail: &str, result: &Result<RestackRun>) -> Option<UndoResult> {
    match result.as_ref().map(|r| &r.outcome) {
        Ok(Outcome::Finished { .. }) => before.map(|b| UndoResult::Recorded(undo::restacked(cwd, b, detail))),
        Ok(Outcome::Stopped { .. }) => Some(UndoResult::Recorded(None)),
        Ok(Outcome::Failed { .. }) | Err(_) => match before {
            Some(b) if !undo::restack_unmoved(cwd, b) => None,
            _ => Some(UndoResult::Recorded(None)),
        },
    }
}

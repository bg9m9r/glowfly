//! Continuing a Finish stopped at a merge conflict: remembering the steps left for the job's tab
//! (active or parked), following each snapshot (the merge seen in progress, committed, or the
//! steps no longer applying), and running the steps left as one job once the merge is committed.

use super::ops::{JobSpec, Started, busy, start_job};
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg};
use crate::state::AppState;
use crate::state::gitflow::{FinishJob, NOT_SET_UP};
use crate::state::gitflow_resume::{StoppedFinish, gone_text, not_committed_text};
use crate::state::tabs::is_repo_at;
use glowfly_core::gitflow::{Branches, ResumeState, StartTips, WorkState, check_resume, resume_state};
use std::path::PathBuf;
use std::time::Instant;

/// What a job's end does to the remembered steps.
pub(super) enum Change {
    Set(StoppedFinish),
    Clear,
    /// Clear them when they belong to this flow branch (it was finished).
    ClearFor(String),
}

/// Applies `change` to the tab of the job's repository `workdir`: the active one (also when the
/// job had no repository), else that tab's parked state.
pub(super) fn store(st: &mut AppState, workdir: Option<PathBuf>, change: Change) {
    let active = workdir.as_ref().is_none_or(|w| st.snap.as_ref().is_some_and(|s| is_repo_at(s, w)));
    let slot = if active {
        &mut st.gitflow_resume
    } else {
        let w = workdir.unwrap_or_default();
        match st.tabs.iter_mut().find(|t| t.is(&w)).and_then(|t| t.parked.as_mut()) {
            Some(p) => &mut p.gitflow_resume,
            None => return,
        }
    };
    match change {
        Change::Set(r) => *slot = Some(r),
        Change::Clear => *slot = None,
        Change::ClearFor(branch) => {
            slot.take_if(|r| r.resume.plan.branch == branch);
        }
    }
}

/// A new snapshot (or a tab brought back): note a merge in progress, offer Continue once the
/// merge is committed, or drop the steps (logged) when they no longer apply. Waits while a
/// Finish job runs: its end decides.
pub(super) fn snapshot_changed(st: &mut AppState) {
    if st.gitflow_finish.is_some() {
        return;
    }
    let (Some(r), Some(snap)) = (st.gitflow_resume.as_mut(), st.snap.as_ref()) else { return };
    r.seen_merge |= r.resume.merging(snap);
    match resume_state(&r.resume, r.seen_merge, snap) {
        ResumeState::Waiting => r.ready = false,
        ResumeState::Ready => r.ready = true,
        ResumeState::Gone(g) => {
            let text = gone_text(st.settings.lang(), &r.resume, &g);
            st.gitflow_resume = None;
            st.log(false, text);
        }
    }
}

/// Continue finish: checks the state again and runs the steps left as one job. Refusals are
/// logged and keep the steps; the job's end replaces or clears them.
pub(super) fn continue_finish(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let (Some(r), Some(snap)) = (st.gitflow_resume.as_ref(), st.snap.as_ref()) else {
        st.log(false, t.tr("gitflow.resume.nothing"));
        return;
    };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    match resume_state(&r.resume, r.seen_merge, snap) {
        ResumeState::Ready => {}
        ResumeState::Waiting => {
            let text = not_committed_text(t, &r.resume.into);
            st.log(false, text);
            return;
        }
        ResumeState::Gone(g) => {
            let text = gone_text(t, &r.resume, &g);
            st.gitflow_resume = None;
            st.log(false, text);
            return;
        }
    }
    let plan = match check_resume(&r.resume, &s, &Branches::of(snap), &WorkState::of(snap)) {
        Ok(plan) => plan,
        Err(p) => {
            st.log(false, p.message());
            return;
        }
    };
    let tips = StartTips::of(snap, &plan);
    if busy(st, false) {
        return;
    }
    if st.prefs.is_some() {
        fx.extend(super::update(st, Msg::Prefs(PrefsMsg::ClosePrefs), now));
    }
    let label = plan.resume_label();
    if start_job(st, JobSpec::new(label.clone(), plan.cmds()), fx) != Started::No {
        let done = plan.summary(&s);
        st.gitflow_finish = Some(FinishJob { label, plan, done, tips, resuming: true, stashed: false });
    }
}

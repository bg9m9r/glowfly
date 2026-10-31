//! Git Flow Finish: opening the dialog for a feature, release or hotfix branch, running the
//! planned merges, tag and delete as one git job (it stops at the first failing step), and the
//! headline when that job (or a Continue finish job) ends: the plan's summary, or which step
//! stopped it and what is left, remembered for Continue finish when a merge conflicted
//! (gitflow_resume.rs). With Stash changes and finish an auto-stash runs first, and Stash and
//! carry on (update/auto_stash.rs) puts the changes back on the branch Finish ends on.

use super::gitflow_resume::{Change, store};
use super::ops::{JobSpec, Started, busy, last_line, start_job};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::auto_stash::blocks;
use crate::state::gitflow::NOT_SET_UP;
use crate::state::gitflow::{FinishForm, FinishJob};
use crate::state::gitflow_resume::StoppedFinish;
use crate::state::{AppState, Modal};
use glowfly_core::gitflow::{
    Branches, FinishOptions, FinishProblem, StartTips, WorkState, after_commands, check_finish, default_tag, default_tag_message,
    failed_step, flow_kind, short_name, stopped_by_conflict,
};
use glowfly_core::i18n::Lang;
use glowfly_core::stash_ops::AutoStashFor;
use std::time::Instant;

/// Opens the Finish dialog for `branch`, unless Git Flow isn't set up or the branch isn't a
/// local flow branch (said in the log). Other problems show in the dialog.
pub(super) fn dialog(st: &mut AppState, branch: &str) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    let Some(kind) = flow_kind(&s, branch) else {
        st.log(false, FinishProblem::NotFlowBranch(branch.to_owned()).message());
        return;
    };
    if !Branches::of(snap).local(branch) {
        st.log(false, FinishProblem::NoBranch(branch.to_owned()).message());
        return;
    }
    let short = short_name(&s, kind, branch);
    st.modal = Some(Modal::GitFlowFinish(FinishForm {
        kind,
        branch: branch.to_owned(),
        tag: default_tag(&s, kind, branch).unwrap_or_default(),
        message: default_tag_message(kind, short),
        keep: false,
        focus_tag: kind.versioned(),
        touched: Default::default(),
    }));
}

/// Checks `branch` and the options against the current snapshot and starts the job. A refusal
/// is logged and leaves the dialog open. With `stash_wanted` (Stash changes and finish) and
/// changes the auto-stash can take, they are stashed first; a clean tree runs a plain Finish.
pub(super) fn finish(st: &mut AppState, branch: &str, options: FinishOptions, stash_wanted: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    let stash = stash_wanted && blocks(snap);
    let mut work = WorkState::of(snap);
    if stash {
        work.tracked_changes = false;
    }
    let plan = match check_finish(&s, branch, &options, &Branches::of(snap), &work) {
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
    if matches!(st.modal, Some(Modal::GitFlowFinish(_))) {
        st.modal = None;
    }
    if st.prefs.is_some() {
        fx.extend(super::update_sealed(st, Msg::Prefs(PrefsMsg::ClosePrefs), now));
    }
    let label = plan.label();
    let mut spec = JobSpec::new(label.clone(), plan.cmds());
    if stash {
        super::auto_stash::wrap(st, &mut spec, AutoStashFor::GitFlowFinish { branch: plan.branch.clone() });
    }
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if start_job(st, spec, fx) != Started::No {
        let done = plan.summary(&s);
        st.gitflow_finish = Some(FinishJob { label, plan, done, tips, resuming: false, stashed: stash });
    }
}

/// The headline of the job that just ended, when it is the pending Finish (or Continue finish)
/// job. Always clears the pending job. A success clears the remembered steps of that branch; a
/// conflict with steps after it remembers them for the job's tab; any other failure of a
/// Continue finish drops them.
pub(crate) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.gitflow_finish.take()?;
    let workdir = match st.job.as_ref() {
        Some(j) if j.label == pending.label => j.workdir.clone(),
        _ => return None,
    };
    let FinishJob { label, plan, done, tips, resuming, stashed } = pending;
    if ok {
        store(st, workdir, Change::ClearFor(plan.branch.clone()));
        return Some(done);
    }
    // The auto-stash's own output comes first: the plan's steps start after it.
    let output = if stashed {
        match after_commands(output, 1) {
            Some(rest) => rest,
            None => {
                let t = st.settings.lang();
                return Some(stash_failed(t, &label, &super::job_errors::reason(t, output)));
            }
        }
    } else {
        output
    };
    let failed = failed_step(output, plan.steps.len());
    let resume = tips.as_ref().filter(|_| stopped_by_conflict(output)).and_then(|t| plan.rest_after_conflict(failed, t));
    let resumable = resume.is_some();
    match resume {
        Some(resume) => store(st, workdir, Change::Set(StoppedFinish { resume, seen_merge: false, ready: false })),
        None if resuming => store(st, workdir, Change::Clear),
        None => {}
    }
    let steps: Vec<String> = plan.steps.into_iter().map(|x| x.what).collect();
    Some(stopped(st.settings.lang(), &label, &steps, output, resumable))
}

/// The headline when the auto-stash before a Finish failed, so none of its steps ran.
pub(super) fn stash_failed(t: Lang, label: &str, why: &str) -> String {
    t.trf("gitflow.finish.stash_failed", &[("label", &label), ("why", &why)])
}

/// Which step stopped the job and what is still to do (`resumable`: the steps after a conflicted
/// merge were remembered for Continue finish).
pub(crate) fn stopped(t: Lang, label: &str, steps: &[String], output: &str, resumable: bool) -> String {
    let i = failed_step(output, steps.len());
    let failed = steps.get(i).map(String::as_str).unwrap_or("");
    let rest = steps.get(i + 1..).unwrap_or_default();
    let todo = if rest.is_empty() { String::new() } else { format!(" Still to do: {}.", rest.join(", ")) };
    if failed.starts_with("merge ") && stopped_by_conflict(output) {
        let then =
            if resumable { "Resolve them, commit the merge, then use Continue finish." } else { "Resolve them and commit the merge." };
        format!("{label} stopped: \"{failed}\" has conflicts. {then}{todo}")
    } else {
        // The cause in plain words, else git's own error line rather than the hints after it.
        let error = || output.lines().rfind(|l| l.starts_with("error: ") || l.starts_with("fatal: ")).unwrap_or_else(|| last_line(output));
        let why = crate::state::error_words::explained(t, output).unwrap_or_else(|| error().to_owned());
        let why = why.trim_end_matches('.');
        format!("{label} stopped at \"{failed}\": {why}.{todo}")
    }
}

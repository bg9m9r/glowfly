//! Git Flow Finish: opening the dialog for a feature, release or hotfix branch, running the
//! planned merges, tag and delete as one git job (it stops at the first failing step), and the
//! headline when that job ends: the plan's summary, or which step stopped it and what is left.

use super::gitflow::NOT_SET_UP;
use super::ops::{JobSpec, busy, last_line, start_job};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::gitflow::{FinishForm, FinishJob};
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::gitflow::{
    Branches, FinishOptions, FinishProblem, WorkState, check_finish, default_tag, default_tag_message, flow_kind, short_name,
};

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
    }));
}

/// Checks `branch` and the options against the current snapshot and starts the job. A refusal
/// is logged and leaves the dialog open.
pub(super) fn finish(st: &mut AppState, branch: &str, options: FinishOptions, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    let plan = match check_finish(&s, branch, &options, &Branches::of(snap), &WorkState::of(snap)) {
        Ok(plan) => plan,
        Err(p) => {
            st.log(false, p.message());
            return;
        }
    };
    if busy(st, false) {
        return;
    }
    if matches!(st.modal, Some(Modal::GitFlowFinish(_))) {
        st.modal = None;
    }
    if st.prefs.is_some() {
        fx.extend(super::update(st, Msg::ClosePrefs, now));
    }
    let label = plan.label();
    start_job(st, JobSpec::new(label.clone(), plan.cmds()), fx);
    if st.job.as_ref().is_some_and(|j| j.label == label) {
        let steps = plan.steps.iter().map(|x| x.what.clone()).collect();
        st.gitflow_finish = Some(FinishJob { label, steps, done: plan.summary(&s) });
    }
}

/// The headline of the job that just ended, when it is the pending Finish job. Always clears
/// the pending job.
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.gitflow_finish.take()?;
    if st.job.as_ref().is_none_or(|j| j.label != pending.label) {
        return None;
    }
    Some(if ok { pending.done } else { stopped(&pending.label, &pending.steps, output) })
}

/// Which step stopped the job and what is still to do. The runner echoes each command it runs
/// as "$ git …", so the number of echoes is the failed step's number.
pub(super) fn stopped(label: &str, steps: &[String], output: &str) -> String {
    let echoes = output.lines().filter(|l| l.starts_with("$ git ")).count();
    let ran = echoes.clamp(1, steps.len().max(1));
    let failed = steps.get(ran - 1).map(String::as_str).unwrap_or("");
    let rest = steps.get(ran..).unwrap_or_default();
    let todo = if rest.is_empty() { String::new() } else { format!(" Still to do: {}.", rest.join(", ")) };
    if failed.starts_with("merge ") && output.contains("CONFLICT") {
        format!("{label} stopped: \"{failed}\" has conflicts. Resolve them and commit the merge.{todo}")
    } else {
        // git's own error line, rather than the hints it prints after it.
        let error = output.lines().rfind(|l| l.starts_with("error: ") || l.starts_with("fatal: "));
        let why = error.unwrap_or_else(|| last_line(output)).trim_end_matches('.');
        format!("{label} stopped at \"{failed}\": {why}.{todo}")
    }
}

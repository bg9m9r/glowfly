//! Cancelling the active job from the status bar. A job with a network command (a fetch, a
//! push, a clone or a pull) gets a stop (`Effect::RunGit::stoppable`); the runner says which kind
//! of command it runs (`JobMsg::Step`), and Cancel works only during a network one: a pull's
//! merge or rebase, and any other command, run to their end (the runner then stops before the
//! next command). The runner stops git's process tree, removes what the stopped command left
//! behind (a fetch's locks, a clone's folder), then reports `JobMsg::Stopped`, and the job ends
//! as cancelled (`logs_runs::job_cancelled`), with no Try again: a cancel was meant. A job that
//! finished before the stop reached it ends as usual. The transfer's progress comes as
//! `JobMsg::Progress` and is kept for the status bar until the step changes.

use crate::effect::partial::StopKey;
use crate::effect::{Effect, UndoResult};
use crate::msg::JobMsg;
use crate::state::AppState;
use crate::state::job_stop::JobStop;
use std::time::Instant;

/// The stop of a job starting with `effect`: one for a stoppable `RunGit`, else none.
pub(super) fn for_effect(effect: &Effect) -> Option<JobStop> {
    match effect {
        Effect::RunGit { stoppable: true, cmds, .. } => Some(JobStop::starting(cmds.first().map(Vec::as_slice))),
        _ => None,
    }
}

/// The status bar's Cancel: asks the runner to stop the active job, once.
pub(super) fn cancel(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.job_cancellable() {
        return;
    }
    let label = st.job.as_ref().map(|j| j.label.clone()).unwrap_or_default();
    if let Some(s) = st.job_stop.as_mut() {
        s.requested = true;
    }
    st.log_quiet(true, format!("Cancelling {label}…"));
    fx.push(Effect::StopGit(StopKey::Job));
}

/// The runner started a kind of command in the active job (`JobMsg::Step`), or git reported its
/// transfer's progress (`JobMsg::Progress`). Dropped when no stoppable job runs.
pub(super) fn live(st: &mut AppState, msg: JobMsg) {
    let Some(s) = st.job_stop.as_mut().filter(|_| st.job.is_some()) else { return };
    match msg {
        JobMsg::Step(step) => {
            s.step = step;
            s.progress = None;
        }
        JobMsg::Progress(p) => s.progress = Some(p),
        _ => {}
    }
}

/// The runner stopped the active job: it ends as "<label> cancelled".
pub(super) fn stopped(st: &mut AppState, output: String, undo: Option<UndoResult>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(label) = st.job.as_ref().map(|j| j.label.clone()) else { return };
    st.job_stop.get_or_insert_with(JobStop::default).stopped = true;
    // Never offered again: running a cancelled push once more is the user's choice to make.
    st.logs.job_retry = None;
    super::partial_content::job_stopped(st);
    let t = st.settings.lang();
    let headline = t.trf("job.headline.cancelled", &[("job", &crate::state::job_spec::labels::shown(t, &label))]);
    super::ops::finish_job(st, false, output, Some(headline), undo, now, fx);
}

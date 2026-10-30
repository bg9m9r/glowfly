//! Cancelling the active job from the status bar: only a job whose every command fetches can be
//! stopped (`Effect::RunGit::stoppable`). The runner stops git's process tree and removes what
//! the stopped fetch left behind, then reports `JobMsg::Stopped`, and the job ends as cancelled
//! (`logs_runs::job_ended`). A job that finished before the stop reached it ends as usual.

use crate::effect::Effect;
use crate::effect::partial::StopKey;
use crate::state::AppState;
use crate::state::job_stop::JobStop;
use std::time::Instant;

/// The stop of a job starting with `effect`: one for a stoppable `RunGit`, else none.
pub(super) fn for_effect(effect: &Effect) -> Option<JobStop> {
    matches!(effect, Effect::RunGit { stoppable: true, .. }).then(JobStop::default)
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

/// The runner stopped the active job: it ends as "<label> cancelled".
pub(super) fn stopped(st: &mut AppState, output: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(label) = st.job.as_ref().map(|j| j.label.clone()) else { return };
    st.job_stop.get_or_insert_with(JobStop::default).stopped = true;
    super::partial_content::job_stopped(st);
    let t = st.settings.lang();
    let headline = t.trf("job.headline.cancelled", &[("job", &crate::state::job_spec::labels::shown(t, &label))]);
    super::ops::finish_job(st, false, output, Some(headline), None, now, fx);
}

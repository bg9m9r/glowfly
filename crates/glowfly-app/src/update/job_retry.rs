//! Try again: a failed job's spec run once more (`JobMsg::Retry`). `start_job` keeps the spec of
//! every job it starts, other than a quiet one or one with its own auto-stash, with the tab it
//! started in (`LogsState::job_retry`); a failure takes it (`take`) and offers it as Try again on
//! lock-file and HTTPS sign-in failures (update/job_errors.rs, `after_headline`), and Delete lock
//! file runs it after deleting the lock (update/lock_file.rs). It runs only in that tab, while it
//! is the active one and still shows the job's repository: a closed tab means no retry.

use super::jobs::start_job;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg};
use crate::state::job_spec::Retry;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::git_errors::{Cause, classify};
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use std::time::Instant;

/// The spec of `job`, which just ended, when `start_job` kept one for it.
pub(super) fn take(st: &mut AppState, job: &RunningJob) -> Option<Retry> {
    st.logs.job_retry.take().filter(|r| r.spec.label == job.label)
}

/// The Try again button that runs `retry`.
pub(super) fn button(t: Lang, retry: &Retry) -> ToastAction {
    let msg = Box::new(Msg::Job(JobMsg::Retry(Box::new(retry.clone()))));
    ToastAction { label: t.tr("job_retry.button").to_owned(), msg, workdir: retry.workdir.clone() }
}

/// Try again for a failure whose toast has its own headline (a cancelled or timed-out sign-in
/// prompt, say), when git's output shows a lock file or an HTTPS sign-in in the way.
pub(super) fn after_headline(t: Lang, retry: Option<&Retry>, output: &str) -> Option<ToastAction> {
    let again = matches!(classify(output), Some(Cause::Locked { .. } | Cause::SignIn(AuthNeed::Https)));
    retry.filter(|_| again).map(|r| button(t, r))
}

/// Why `retry` can't run now: its tab is closed, isn't the active one, or shows another
/// repository. None when it may.
pub(super) fn refusal(st: &AppState, retry: &Retry) -> Option<String> {
    let t = st.settings.lang();
    let label = retry.spec.label.as_str();
    let job: [(&str, &dyn std::fmt::Display); 1] = [("job", &label)];
    if !st.tabs.iter().any(|tab| tab.id == retry.tab) {
        return Some(t.trf("job_retry.tab_closed", &job));
    }
    if st.tabs.get(st.active_tab).map(|tab| tab.id) != Some(retry.tab) {
        return Some(t.trf("job_retry.switch_back", &job));
    }
    let here = match (&retry.workdir, st.workdir()) {
        (Some(w), Some(a)) => same_spelling(w, &a),
        (w, a) => w.is_none() && a.is_none(),
    };
    (!here).then(|| t.trf("job_retry.moved", &job))
}

/// Runs `retry`'s spec again, unless `refusal` says why not (in a toast).
pub(super) fn run(st: &mut AppState, retry: Retry, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(why) = refusal(st, &retry) {
        st.toast(false, why, now);
        return;
    }
    start_job(st, retry.spec, fx);
}

//! Try again: a failed job's spec run once more (`JobMsg::Retry`). `start_job` keeps the spec of
//! every job it starts, other than a quiet one or one with its own auto-stash, with the tab it
//! started in (`LogsState::job_retry`); a failure takes it (`take`) and offers it as Try again on
//! lock-file and HTTPS sign-in failures (update/job_errors.rs, `after_headline`), and Delete lock
//! file runs it after deleting the lock (update/lock_file.rs). It runs only in that tab, while it
//! is the active one and still shows the job's repository: a closed tab means no retry. A job
//! of several commands is offered only when it failed in its first one, so nothing that already
//! ran runs twice; a commit is made again from the draft as it is now.

use super::jobs::start_job;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg};
use crate::state::job_spec::{JobSpec, Retry};
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::git_errors::{Cause, classify};
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use std::time::Instant;

/// The spec of `job`, which just ended with `output`, when `start_job` kept one for it and
/// running it again repeats nothing that already ran (`failed_first`).
pub(super) fn take(st: &mut AppState, job: &RunningJob, output: &str) -> Option<Retry> {
    st.logs.job_retry.take().filter(|r| r.spec.label == job.label && failed_first(&r.spec, output))
}

/// Whether the job stopped in its first command: always for a single command, and for more when
/// `output` echoes no more than one `$ git ` command (counted as `gitflow::finish::failed_step`
/// does), leaving out the failure check run after it (`on_fail_check`). Counting too many only
/// withholds Try again.
fn failed_first(spec: &JobSpec, output: &str) -> bool {
    if spec.cmds.len() <= 1 {
        return true;
    }
    let echoes: Vec<&str> = output.lines().filter(|l| l.starts_with("$ git ")).collect();
    let check = spec.on_fail_check.as_ref().map(|c| format!("$ git {}", c.join(" ")));
    let checked = check.is_some_and(|c| echoes.last().is_some_and(|l| *l == c));
    echoes.len() - usize::from(checked) <= 1
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
    let label = crate::state::job_spec::labels::shown(t, &retry.spec.label);
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
    if retry.spec.clear_commit {
        // A commit is made from the draft as it is now (message, amend, push after): its old spec
        // would commit the message it had and then clear what was written since.
        let stage_all = retry.spec.cmds.first().is_some_and(|c| c.first().is_some_and(|a| a == "add"));
        super::commit::commit(st, stage_all, now, fx);
        return;
    }
    start_job(st, retry.spec, fx);
}

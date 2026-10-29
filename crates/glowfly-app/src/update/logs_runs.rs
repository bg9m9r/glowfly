//! Finished jobs in the activity log: each is logged as a run (its label, a one-line summary
//! and how long it took), a loud failure opens its row and keeps its fix, which the panel's
//! button and the failure toast's share (`LogsMsg::Fix`), and the panel's rows fold open and
//! closed (`LogsMsg::Fold`). `stamp` times the active job from the end of the `update` that
//! started it.

use super::job_errors;
use crate::effect::{Effect, NoteKind};
use crate::logs::LogsMsg;
use crate::msg::{JobMsg, Msg};
use crate::state::activity_rows::is_routine_label;
use crate::state::auto_stash::StashRetry;
use crate::state::logs::EntryId;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::logs::{RunInfo, last_message};
use std::time::Instant;

/// Remembers when the active job started: the first `update` that ends with it running. A job
/// held for the trust prompt isn't running, so it is timed from its release.
pub(super) fn stamp(st: &mut AppState, now: Instant) {
    st.logs.job_started = st.job.as_ref().map(|_| st.logs.job_started.unwrap_or(now));
}

/// Logs the job that just ended as a run and toasts it (`headline`; for a failure without one,
/// git's cause in plain words with its fix, update/job_errors.rs; else "<label> succeeded" /
/// "<label> failed: <reason>"). A quiet job (auto-fetch) is only logged.
#[allow(clippy::too_many_arguments)]
pub(super) fn job_ended(
    st: &mut AppState,
    job: &RunningJob,
    ok: bool,
    output: &str,
    headline: Option<String>,
    stash_retry: Option<StashRetry>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let took_ms = st.logs.job_started.take().map(|s| u64::try_from(now.saturating_duration_since(s).as_millis()).unwrap_or(u64::MAX));
    let status = if ok { "done" } else { "FAILED" };
    let text = format!("{} {status}\n{output}", job.label);
    let run = |summary| RunInfo { title: job.label.clone(), summary, took_ms, routine: is_routine_label(&job.label) };
    let repo = job.workdir.as_deref();
    let again = super::job_retry::take(st, job, output);
    if job.quiet {
        let explained = (!ok).then(|| job_errors::explain(st, job, stash_retry, None, output)).flatten();
        let summary = explained.map_or_else(|| last_message(output), |e| e.text);
        st.log_run_in(repo, ok, text, run(summary), false);
        return;
    }
    let explained = (!ok && headline.is_none()).then(|| job_errors::explain(st, job, stash_retry, again.as_ref(), output)).flatten();
    let summary = match (&headline, &explained) {
        (Some(h), _) => h.clone(),
        (None, Some(e)) => e.text.clone(),
        (None, None) => last_message(output),
    };
    let (headline, fix) = match (headline, explained) {
        (Some(h), _) => (h, (!ok).then(|| super::job_retry::after_headline(st.settings.lang(), again.as_ref(), output)).flatten()),
        (None, Some(e)) => (e.text, e.fix),
        (None, None) => (job_errors::default_headline(&job.label, ok, output), None),
    };
    super::notify::desktop(st, NoteKind::Job, &headline, repo, fx);
    // Pushed first, so a failure's announcement gives this toast its Details.
    st.toast(ok, headline, now);
    let id = st.log_run_in(repo, ok, text, run(summary), true);
    if !ok {
        st.logs.open.insert(id);
    }
    if let Some(f) = fix {
        let retrying = matches!(*f.msg, Msg::Job(JobMsg::Retry(_)));
        let proxy = st.logs.keep_fix(id, f);
        st.set_last_toast_action(proxy);
        // A lock file in the way may be offered for deleting, once the runner has looked.
        if retrying {
            super::lock_file::check_after(job, id, output, fx);
        }
    }
}

/// Logs the job the user cancelled as a run ("<label> cancelled", summary "Cancelled") with a
/// plain toast (`headline`): not a failure, so no desktop note and its row stays folded.
pub(super) fn job_cancelled(st: &mut AppState, job: &RunningJob, output: &str, headline: String, now: Instant) {
    let took_ms = st.logs.job_started.take().map(|s| u64::try_from(now.saturating_duration_since(s).as_millis()).unwrap_or(u64::MAX));
    let text = format!("{} cancelled\n{output}", job.label);
    let run = RunInfo { title: job.label.clone(), summary: "Cancelled".to_owned(), took_ms, routine: is_routine_label(&job.label) };
    if !job.quiet {
        st.toast(true, headline, now);
    }
    st.log_run_in(job.workdir.as_deref(), true, text, run, false);
}

/// Whether `action` is a toast button that runs one of run `id`'s fixes.
pub(super) fn is_proxy(action: Option<&ToastAction>, id: EntryId) -> bool {
    action.is_some_and(|a| matches!(*a.msg, Msg::Logs(LogsMsg::Fix(i) | LogsMsg::AltFix(i)) if i == id))
}

/// Runs run `id`'s fix (with `alt`, its second fix) once, while its repository is the active
/// tab (else a toast says to switch back and the fix is kept). The other fix and the failure
/// toast offering them go too.
pub(super) fn fix(st: &mut AppState, id: EntryId, alt: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(f) = (if alt { st.logs.alt_fix(id) } else { st.logs.fix(id) }) else { return };
    // Checked on a stand-in, so a refusal leaves the fix where it is.
    let probe = ToastAction { label: f.label.clone(), msg: Box::new(Msg::Logs(LogsMsg::Fix(id))), workdir: f.workdir.clone() };
    if !super::notify::may_run(st, &probe, now) {
        return;
    }
    let Some(action) = st.logs.take_fix(id, alt) else { return };
    st.toasts.retain(|t| ![&t.action, &t.alt, &t.details].iter().any(|a| is_proxy(a.as_ref(), id)));
    super::notify::run_action(st, action, now, fx);
}

/// Folds an activity row (a routine group by its first entry) open, or closed again.
pub(super) fn fold(st: &mut AppState, id: EntryId) {
    if !st.logs.open.remove(&id) {
        st.logs.open.insert(id);
    }
}

//! The "Saved" tick of the Preferences pages whose Save runs a git job (Git Flow, sparse
//! checkout): recorded when the job starts or is held (`started`), marked when that job
//! succeeds and the page shows its reason when it fails (`job_ended`, from ops::finish_job),
//! recorded again when Try again runs it once more (`retried`), forgotten when it is cancelled,
//! runs for another repository than the one shown, or is dropped at the trust prompt (`dropped`,
//! from trust::forget).

use crate::state::prefs_save::JobSave;
use crate::state::{AppState, PrefPage, RunningJob};
use std::path::Path;

/// The page's Save started (or holds for the trust prompt) the job `label`.
pub(super) fn started(st: &mut AppState, page: PrefPage, label: &str) {
    let workdir = st.workdir();
    if let Some(p) = st.prefs.as_mut() {
        p.job_save = Some(JobSave { page, label: label.into(), workdir });
    }
}

/// Try again or Delete lock file runs the failed save's job once more (update/job_retry.rs): it is
/// the page's save again, so its ending ticks or shows its reason like the first run's did. Only
/// a Git Flow save can be run again (a sparse save has no retry).
pub(super) fn retried(st: &mut AppState, label: &str) {
    use crate::state::gitflow::{INIT_LABEL, SAVE_LABEL};
    if let Some(label) = [INIT_LABEL, SAVE_LABEL].into_iter().find(|l| *l == label) {
        started(st, PrefPage::RepoGitFlow, label);
    }
}

/// A job ended: if it is the recorded save's, the tick shows on success and the page shows git's
/// reason on failure. A cancelled job, or one for another repository than the one shown, says
/// nothing here (its own toast does).
pub(super) fn job_ended(st: &mut AppState, job: &RunningJob, ok: bool, cancelled: bool, output: &str) {
    let t = st.settings.lang();
    let active = st.workdir();
    let Some(p) = st.prefs.as_mut() else { return };
    let ours = |s: &JobSave| s.label == job.label && same(s.workdir.as_deref(), job.workdir.as_deref());
    let Some(save) = p.job_save.take_if(|s| ours(s)) else { return };
    if cancelled || !same(save.workdir.as_deref(), active.as_deref()) {
        return;
    }
    if ok {
        if p.save_error.as_ref().is_some_and(|(at, _)| *at == save.page) {
            p.save_error = None;
        }
        p.saved.mark();
    } else {
        p.save_error = Some((save.page, super::job_errors::reason(t, output)));
    }
}

/// The held job `label` was dropped without running: its save ends with no tick and no error.
pub(super) fn dropped(st: &mut AppState, label: &str) {
    if let Some(p) = st.prefs.as_mut() {
        p.job_save.take_if(|s| s.label == label);
    }
}

fn same(a: Option<&Path>, b: Option<&Path>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => glowfly_core::paths::same_spelling(a, b),
        _ => false,
    }
}

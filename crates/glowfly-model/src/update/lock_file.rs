//! Delete lock file. A job that failed on a lock file git left behind offers Try again at once
//! (update/job_retry.rs) and asks the runner whether the lock may go (`check_after`): only when
//! it is a plain file in the repository's git dir and no git process runs
//! (`glowfly_core::lock_file`) does Delete lock file come first on the run and its toast
//! (`checked`). The button asks first, then the runner checks everything again and deletes it
//! (`delete`), and the job runs again (`deleted`); a refusal says why and still offers Try again.

use crate::effect::{Effect, LockEffect};
use crate::msg::{JobMsg, LockMsg, Msg};
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::git_procs::Unknown;
use glowfly_core::i18n::Lang;
use glowfly_core::lock_file::{Found, Refusal};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: LockMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LockMsg::Checked { entry, workdir, lock, result } => checked(st, entry, workdir, lock, result),
        LockMsg::Delete { workdir, lock, retry } => delete(st, workdir, lock, retry, now, fx),
        LockMsg::Deleted { lock, retry, result } => deleted(st, &lock, *retry, result, now, fx),
    }
}

/// After run `entry` of `job` failed and kept Try again as its fix: when git's `output` names a
/// lock file in the job's repository, asks whether it may be deleted.
pub(super) fn check_after(job: &RunningJob, entry: EntryId, output: &str, fx: &mut Vec<Effect>) {
    let (Some(workdir), Some(lock)) = (job.workdir.clone(), glowfly_core::git_errors::lock_path(output)) else { return };
    fx.push(Effect::Lock(LockEffect::Check { workdir, lock, entry }));
}

/// The file name of `lock`, for the texts.
fn file_name(lock: &str) -> &str {
    lock.rsplit(['/', '\\']).next().unwrap_or(lock)
}

/// The check found a stale lock: Delete lock file goes first on run `entry` (and its toast,
/// while that is up), with its Try again beside it. Anything else leaves Try again alone.
fn checked(st: &mut AppState, entry: EntryId, workdir: PathBuf, lock: String, result: Result<Found, Refusal>) {
    let Ok(Found::Stale(_)) = result else { return };
    // The run's Try again, while it hasn't run.
    let Some(Msg::Job(JobMsg::Retry(retry))) = st.logs.fix(entry).map(|f| &*f.msg) else { return };
    let retry = retry.clone();
    let t = st.settings.lang();
    let (file, job) = (file_name(&lock), retry.spec.label.clone());
    let path = glowfly_core::native_path::shown(Path::new(&lock));
    let body = t.trf("lock.delete.body", &[("file", &file), ("job", &job), ("path", &path)]);
    let then = Msg::Lock(LockMsg::Delete { workdir, lock: lock.clone(), retry: retry.clone() });
    let ask = Msg::confirm_danger(t.tr("lock.delete.title"), body, t.tr("lock.delete.confirm"), then);
    let action = ToastAction { label: t.tr("lock.delete.button").to_owned(), msg: Box::new(ask), workdir: retry.workdir.clone() };
    let Some((main, alt)) = st.logs.offer_first(entry, action) else { return };
    let shown = st.toasts.iter_mut().find(|t| super::logs_runs::is_proxy(t.action.as_ref(), entry));
    if let Some(toast) = shown {
        (toast.action, toast.alt) = (Some(main), Some(alt));
    }
}

/// Delete lock file, confirmed: refused while a job runs or once the job's tab is closed or
/// not active; otherwise the runner checks again and deletes it.
fn delete(st: &mut AppState, workdir: PathBuf, lock: String, retry: Box<Retry>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let file = file_name(&lock).to_owned();
    if let Some(job) = &st.job {
        let text = t.trf("lock.refused.busy", &[("file", &file), ("job", &job.label)]);
        return refused(st, text, &retry, now);
    }
    if let Some(why) = super::job_retry::refusal(st, &retry) {
        return st.toast(false, why, now);
    }
    fx.push(Effect::Lock(LockEffect::Delete { workdir, lock, retry }));
}

/// The delete ended: logs it and runs the job again, or says why it wasn't deleted.
fn deleted(st: &mut AppState, lock: &str, retry: Retry, result: Result<Found, Refusal>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let file = file_name(lock);
    match result {
        Ok(found) => {
            if matches!(found, Found::Stale(_)) {
                st.log(true, t.trf("lock.deleted", &[("file", &file)]));
            }
            super::job_retry::run(st, retry, now, fx);
        }
        Err(why) => {
            let text = refusal_text(t, file, &why);
            refused(st, text, &retry, now);
        }
    }
}

/// A refusal's toast, with Try again.
fn refused(st: &mut AppState, text: String, retry: &Retry, now: Instant) {
    let again = super::job_retry::button(st.settings.lang(), retry);
    st.toast_action(false, text, again, now);
}

/// Why lock `file` wasn't deleted, in words.
pub(super) fn refusal_text(t: Lang, file: &str, why: &Refusal) -> String {
    let f: [(&str, &dyn std::fmt::Display); 1] = [("file", &file)];
    match why {
        Refusal::NotLock | Refusal::Outside | Refusal::NotAFile => t.trf("lock.refused.place", &f),
        Refusal::GitRunning(found) => {
            let programs: Vec<String> = found.iter().map(|(pid, name)| format!("{name} ({pid})")).collect();
            t.trf("lock.refused.running", &[("file", &file), ("programs", &programs.join(", "))])
        }
        Refusal::Unknown(Unknown::NoList) => t.trf("lock.refused.no_list", &f),
        Refusal::Unknown(Unknown::Unreadable) => t.trf("lock.refused.unreadable", &f),
        Refusal::Unknown(Unknown::Hidden) => t.trf("lock.refused.hidden", &f),
        Refusal::Unknown(Unknown::Sandbox) => t.trf("lock.refused.sandbox", &f),
        Refusal::Unknown(Unknown::Wsl) => t.trf("lock.refused.wsl", &f),
        Refusal::Failed(e) => t.trf("lock.refused.failed", &[("file", &file), ("error", e)]),
    }
}

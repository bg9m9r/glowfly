//! The help a TLS failure gets. On a revocation check Windows couldn't finish, Relax check for
//! this repository and for all repositories (`offers`) each ask first, then set
//! `http.schannelCheckRevoke` to best-effort (`git config --local` or `--global`) and run the
//! failed job again (update/job_retry.rs); a git older than 2.43 gets no fix, only why. After a
//! loud certificate or revocation failure the runner reads the TLS settings git used
//! (`read_after`), which the run's Details then show (state/tls_help.rs).

use crate::effect::{Effect, TlsEffect};
use crate::msg::{Msg, TlsMsg};
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use crate::state::tls_help::{self, TlsNote};
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::git_version;
use glowfly_core::tls_config::{TlsFailure, best_effort_supported};
use std::path::PathBuf;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: TlsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        TlsMsg::SettingsRead { entry, failure, result } => {
            tls_help::keep(&mut st.logs, entry, TlsNote { failure, settings: result.map(|s| *s) })
        }
        TlsMsg::SetBestEffort { workdir, global, retry } => fx.push(Effect::Tls(TlsEffect::SetBestEffort { cwd: workdir, global, retry })),
        TlsMsg::BestEffortSet { global, retry, result } => {
            let t = st.settings.lang();
            match result {
                Ok(()) => {
                    let text = if global { t.tr("tls.revoke.set.global") } else { t.tr("tls.revoke.set.local") };
                    st.log(true, text);
                    match retry {
                        Some(r) => super::job_retry::run(st, *r, now, fx),
                        None => st.toast(true, text, now),
                    }
                }
                Err(e) => st.toast(false, t.trf("tls.revoke.failed", &[("error", &e.said)]), now),
            }
        }
    }
}

/// The fixes a revocation failure's toast offers: Relax check for this repository first, for
/// all repositories beside it; `needs_git` instead says why there are none.
pub(super) struct Offers {
    pub main: Option<ToastAction>,
    pub alt: Option<ToastAction>,
    pub needs_git: Option<String>,
}

/// What `job`'s revocation failure offers; `again` is its spec, run again once the value is
/// set. The fix for this repository needs the job's own repository (not a clone's or an
/// init's); neither is offered for a git known to be older than 2.43, which would die on the
/// value. A quiet job (auto-fetch) only logs the sentence: nothing is offered.
pub(super) fn offers(st: &AppState, job: &RunningJob, again: Option<&Retry>) -> Offers {
    if job.quiet {
        return Offers { main: None, alt: None, needs_git: None };
    }
    let t = st.settings.lang();
    let version = st.git.info.as_ref().and_then(|i| i.version);
    if let Some(v) = version.filter(|v| !best_effort_supported(Some(*v))) {
        let needs_git = t.trf("tls.revoke.needs_git", &[("version", &git_version::show(v))]);
        return Offers { main: None, alt: None, needs_git: Some(needs_git) };
    }
    let ask = |global: bool, workdir: PathBuf| {
        let mut body = if global { t.tr("tls.revoke.body.global") } else { t.tr("tls.revoke.body.local") }.to_owned();
        if let Some(r) = again {
            let job = crate::state::job_spec::labels::shown(t, &r.spec.label);
            body = format!("{body}\n\n{}", t.trf("tls.revoke.body.retry", &[("job", &job)]));
        }
        let then = Msg::Tls(TlsMsg::SetBestEffort { workdir, global, retry: again.cloned().map(Box::new) });
        let label = if global { t.tr("tls.revoke.global.button") } else { t.tr("tls.revoke.local.button") };
        let msg = Msg::confirm(t.tr("tls.revoke.title"), body, t.tr("tls.revoke.confirm"), then);
        // Both stay with the job's tab, as its other fixes do.
        ToastAction { label: label.to_owned(), msg: Box::new(msg), workdir: job.workdir.clone() }
    };
    let own = job.workdir.clone().filter(|_| job.then_open.is_none());
    let local = own.map(|w| ask(false, w));
    let global = Some(ask(true, AppState::outside_dir()));
    match local {
        Some(l) => Offers { main: Some(l), alt: global, needs_git: None },
        None => Offers { main: global, alt: None, needs_git: None },
    }
}

/// After run `entry` of `job` failed with `failure`: reads the TLS settings git used, for the
/// URL its `output` names (no credentials), in the job's repository; a clone's or init's
/// folder may be another tab's, so those read from outside any repository.
pub(super) fn read_after(job: &RunningJob, entry: EntryId, output: &str, failure: TlsFailure, fx: &mut Vec<Effect>) {
    let cwd = job.workdir.clone().filter(|_| job.then_open.is_none()).unwrap_or_else(AppState::outside_dir);
    let url = glowfly_core::remote_failure::transport_url(output);
    fx.push(Effect::Tls(TlsEffect::ReadSettings { cwd, url, entry, failure }));
}

//! Repository trust: asking once whether to trust a repository whose own config or hooks make
//! git run programs, recording the answer, bringing the prompt back on request (the toolbar's
//! Untrusted chip, the palette), and trusting repositories Trellis cloned or created. What is
//! held until then is checked where it starts (`AppState::runs_git_unasked`); jobs the user
//! starts in such a repository wait here for the decision (`waits`/`hold`), run on Trust and
//! run, and are dropped on Cancel.

use super::jobs::Started;
use crate::effect::{Effect, LoadKind};
use crate::msg::TrustMsg;
use crate::state::tabs::is_repo_at;
use crate::state::trust::{Created, HeldJob, Standing, prompt_for, record, standing};
use crate::state::{AppState, Modal, RunningJob};
use std::path::Path;
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::settings::RepoTrust;

pub(super) fn handle(st: &mut AppState, msg: TrustMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        TrustMsg::Decide { real, trusted } => decide(st, &real, trusted, now, fx),
        TrustMsg::Ask { real } => ask(st, &real),
    }
}

/// Shows the prompt again for the active repository at `real`, if git is held there (Undecided
/// or NotTrusted) and no other dialog is open. Closes the palette, which never covers a dialog.
fn ask(st: &mut AppState, real: &Path) {
    let Some(snap) = st.snap.as_ref().filter(|s| same_spelling(&s.real_workdir, real)) else { return };
    if !matches!(standing(&st.settings, snap), Standing::Undecided | Standing::NotTrusted) || st.modal.is_some() {
        return;
    }
    let prompt = prompt_for(&st.settings, snap);
    if !st.trust.asked.iter().any(|p| same_spelling(p, &prompt.real)) {
        st.trust.asked.push(prompt.real.clone());
    }
    st.palette = None;
    st.modal = Some(Modal::Trust(prompt));
}

fn decide(st: &mut AppState, real: &Path, trusted: bool, now: Instant, fx: &mut Vec<Effect>) {
    // Only the prompt that names the held job can release it; any other held job is stale.
    let held = st.trust.held.take();
    let named = matches!(&st.modal, Some(Modal::Trust(p)) if p.held.is_some());
    record(&mut st.settings, real, if trusted { RepoTrust::Trusted } else { RepoTrust::NotTrusted });
    fx.push(super::prefs::save_settings(st));
    if matches!(st.modal, Some(Modal::Trust(_))) {
        st.modal = None;
    }
    let Some(snap) = st.snap.as_ref().filter(|s| same_spelling(&s.real_workdir, real)) else {
        if let Some(h) = held {
            forget(st, h, trusted);
        }
        return;
    };
    let name = snap.name.clone();
    if trusted {
        st.toast(true, format!("{name} is trusted. Background fetch is on."), now);
        // The load options changed (trusted), so this is a full load whose status git reads.
        super::repo::refresh(st, fx);
        super::forge::refresh(st, true, false, now, fx);
        if let Some(h) = held {
            release(st, h, real, named, fx);
        }
    } else {
        st.log(true, format!("{name} isn't trusted. Trellis won't fetch or contact its hosting service for it on its own."));
        if let Some(h) = held {
            forget(st, h, false);
        }
    }
}

/// Starts the held job `h` exactly as it was requested, now that the repository at `real` is
/// trusted, if it was held for that repository, it is still the active one, its prompt was the
/// one decided (`named`) and no other job is running; otherwise drops it.
fn release(st: &mut AppState, h: HeldJob, real: &Path, named: bool, fx: &mut Vec<Effect>) {
    let here = named && same_spelling(&h.real, real) && st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &h.real));
    if !here || super::jobs::busy(st, false) {
        return forget(st, h, true);
    }
    tracing::debug!("running {} now that {} is trusted", h.job.label, h.name);
    st.job = Some(h.job);
    fx.push(h.effect);
}

/// Whether `job`, about to run in `cwd`, must wait for the trust prompt: git is held in the
/// active repository and `cwd` is that repository. Clones and `git init`s Trellis trusts when
/// they open don't wait.
pub(super) fn waits(st: &AppState, job: &RunningJob, cwd: &Path) -> bool {
    st.held()
        && !matches!(job.then_open, Some((_, Created::Clone | Created::Init)))
        && st.snap.as_ref().is_some_and(|s| crate::state::tabs::is_repo_at(s, cwd))
}

/// Keeps `job` and its `effect` as the held job and opens the trust prompt naming it, closing
/// the palette and replacing any other dialog. Pushes no effect. A quiet job is dropped
/// instead, and so is a second job while a held prompt is open.
pub(super) fn hold(st: &mut AppState, job: RunningJob, effect: Effect) -> Started {
    let Some(name) = st.snap.as_ref().map(|s| s.name.clone()) else { return Started::No };
    if job.quiet {
        st.log_quiet(false, format!("{} skipped: {name} isn't trusted.", job.label));
        return Started::No;
    }
    if st.trust.held.is_some() {
        if matches!(&st.modal, Some(Modal::Trust(p)) if p.held.is_some()) {
            st.log(false, format!("{} wasn't run: decide whether to trust {name} first.", job.label));
            return Started::No;
        }
        // Its prompt was closed some other way.
        if let Some(old) = st.trust.held.take() {
            forget(st, old, false);
        }
    }
    let Some(snap) = st.snap.as_ref() else { return Started::No };
    let mut prompt = prompt_for(&st.settings, snap);
    prompt.held = Some(job.label.clone());
    if !st.trust.asked.iter().any(|p| same_spelling(p, &prompt.real)) {
        st.trust.asked.push(prompt.real.clone());
    }
    tracing::debug!("holding {} until {name} is trusted", job.label);
    let real = prompt.real.clone();
    st.palette = None;
    st.modal = Some(Modal::Trust(prompt));
    st.trust.held = Some(HeldJob { real, name, job, effect });
    Started::Held
}

/// The trust prompt was closed without a decision: the held job, if any, is dropped.
pub(super) fn dismissed(st: &mut AppState) {
    if let Some(h) = st.trust.held.take() {
        forget(st, h, false);
    }
}

/// Drops the held job `h`: clears what its caller set up to follow it, in its own tab whether
/// or not that is still the active one, and says it didn't run, and why (`trusted`: the
/// repository is trusted now, but the job couldn't run as asked).
fn forget(st: &mut AppState, h: HeldJob, trusted: bool) {
    let label = h.job.label.as_str();
    st.gitflow_finish.take_if(|j| j.label == label);
    st.tag_job.take_if(|j| j.label == label);
    st.patch_apply.take_if(|j| j.label == label);
    st.auto_stash.making.take_if(|m| m.label == label);
    if label == super::rebase::JOB_LABEL {
        let active = st.snap.as_ref().is_some_and(|s| is_repo_at(s, &h.real));
        let panel = if active {
            st.rebase.as_mut()
        } else {
            st.tabs
                .iter_mut()
                .filter_map(|t| t.parked.as_mut())
                .find(|p| p.snap.as_ref().is_some_and(|s| is_repo_at(s, &h.real)))
                .and_then(|p| p.rebase.as_mut())
        };
        if let Some(p) = panel {
            p.starting = false;
        }
    }
    let text = if trusted {
        format!("{label} wasn't run: {} is trusted now, so start it again.", h.name)
    } else {
        format!("{label} wasn't run. Trellis runs git commands in {} only after you trust it.", h.name)
    };
    st.log(true, text);
}

/// A job that made the repository at `path` succeeded and Trellis is about to open it: a clone,
/// or a `git init` whose `output` says it made a new repository, is trusted when it loads.
pub(super) fn created(st: &mut AppState, path: &Path, how: Created, output: &str) {
    let new = match how {
        Created::Clone => true,
        Created::Init => trellis_core::trust::init_created_new(output),
        Created::Worktree => false,
    };
    if new {
        st.trust.created.push(path.to_path_buf());
    }
}

/// The active tab's snapshot was just replaced by a full load of `kind`.
pub(super) fn loaded(st: &mut AppState, kind: LoadKind, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let made = st.trust.created.iter().position(|p| st.names_repo(snap, p));
    if let Some(i) = made {
        st.trust.created.remove(i);
        let real = snap.real_workdir.clone();
        record(&mut st.settings, &real, RepoTrust::Trusted);
        fx.push(super::prefs::save_settings(st));
    }
    maybe_ask(st, kind == LoadKind::Open);
}

/// Shows the trust prompt for the active repository if it is undecided, no other dialog is
/// open, and it wasn't asked about this session (a `fresh_open` asks again).
pub(super) fn maybe_ask(st: &mut AppState, fresh_open: bool) {
    let Some(snap) = &st.snap else { return };
    if standing(&st.settings, snap) != Standing::Undecided || st.modal.is_some() {
        return;
    }
    let asked = st.trust.asked.iter().any(|p| same_spelling(p, &snap.real_workdir));
    if asked && !fresh_open {
        return;
    }
    tracing::debug!("asking whether to trust {}", snap.real_workdir.display());
    let prompt = prompt_for(&st.settings, snap);
    if !asked {
        st.trust.asked.push(prompt.real.clone());
    }
    st.modal = Some(Modal::Trust(prompt));
}

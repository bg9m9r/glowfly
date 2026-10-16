//! The Sparse Checkout preferences page: reading the worktree's state, and turning sparse
//! checkout on, saving its paths, reapplying them and turning it off, each run as the active
//! git job. These jobs rewrite the working tree, so the tab reloads in full afterwards.

use super::ops;
use crate::effect::Effect;
use crate::msg::SparseMsg;
use crate::state::sparse::{DISABLE_LABEL, REAPPLY_LABEL, kept_headline};
use crate::state::{AppState, PrefPage};
use std::time::Instant;
use trellis_core::sparse_ops::{self, SparseCommand, SparseStatus};

const OFF: &str = "Sparse checkout is off in this repository.";

pub(super) fn handle(st: &mut AppState, m: SparseMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        SparseMsg::ShowPage => {
            if let Some(p) = st.prefs.as_mut() {
                p.page = PrefPage::RepoSparse;
                load(st, fx);
            }
        }
        SparseMsg::Reload => load(st, fx),
        SparseMsg::ResetForm => {
            if let Some(p) = st.prefs.as_mut() {
                p.sparse.fill();
            }
        }
        SparseMsg::Apply => apply(st, fx),
        SparseMsg::Reapply => when_on(st, REAPPLY_LABEL, SparseCommand::Reapply, fx),
        SparseMsg::Disable => when_on(st, DISABLE_LABEL, SparseCommand::Disable, fx),
        SparseMsg::Loaded { generation, result } => loaded(st, generation, result),
        SparseMsg::Done { ok, output } => done(st, ok, output, now, fx),
    }
}

/// Reads the active repository's status for the open page, showing "Reading" meanwhile.
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    read(st, false, fx);
}

/// Asks for the status; `keep` leaves the current one on screen until the new one arrives, so
/// the form (with its text and focus) stays put.
fn read(st: &mut AppState, keep: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Some(p) = st.prefs.as_mut() else { return };
    p.sparse.generation += 1;
    if !keep {
        p.sparse.status = None;
    }
    p.sparse.error = None;
    fx.push(Effect::LoadSparse { workdir, generation: p.sparse.generation });
}

fn status(st: &AppState) -> Option<&SparseStatus> {
    st.prefs.as_ref().and_then(|p| p.sparse.status.as_ref())
}

/// Turn on or Save paths: the form's text, checked for its mode first.
fn apply(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    if p.sparse.status.is_none() {
        return;
    }
    match sparse_ops::parse_paths(&p.sparse.paths, p.sparse.mode) {
        Err(problem) => p.sparse.paths_error = Some(problem.message()),
        Ok(paths) => {
            p.sparse.paths_error = None;
            let (label, mode) = (p.sparse.apply_label(), p.sparse.mode);
            start(st, label, SparseCommand::Set { mode, paths }, fx);
        }
    }
}

/// Reapply and Turn off: git refuses them unless sparse checkout is on.
fn when_on(st: &mut AppState, label: &str, command: SparseCommand, fx: &mut Vec<Effect>) {
    if !status(st).is_some_and(|s| s.enabled) {
        return st.log(false, OFF);
    }
    start(st, label, command, fx);
}

/// Runs `command` as the active job, unless one is running (said in the log). It rewrites the
/// working tree, so the undo history is cleared.
fn start(st: &mut AppState, label: &str, command: SparseCommand, fx: &mut Vec<Effect>) {
    ops::begin_job(st, label, false, move |cwd| Effect::SparseCheckout { cwd, command }, fx);
}

fn loaded(st: &mut AppState, generation: u64, result: Result<SparseStatus, String>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let page = &mut p.sparse;
    if page.generation != generation {
        return;
    }
    let refill = std::mem::take(&mut page.refill);
    match result {
        Ok(s) => {
            page.status = Some(s);
            if refill || !page.edited() {
                page.fill();
            }
        }
        Err(e) => {
            page.status = None;
            page.error = Some(e);
        }
    }
}

/// A sparse job ended: the usual toast and log (or the kept-files headline), then, if its tab
/// is still active, a full reload (the files under the status changed) and a fresh status for
/// the page, refilling the form after a success.
fn done(st: &mut AppState, ok: bool, output: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => crate::state::tabs::same_path(a, &b),
        _ => false,
    };
    let kept = if ok { sparse_ops::left_behind(&output).len() } else { 0 };
    let headline = (kept > 0).then(|| kept_headline(&job.label, kept));
    ops::finish_job(st, ok, output, headline, None, now, fx);
    if !here {
        return;
    }
    // Supersedes the checked refresh finish_job started: skip-worktree bits don't change refs.
    super::refresh::reload(st, fx);
    if let Some(p) = st.prefs.as_mut().filter(|p| p.page == PrefPage::RepoSparse) {
        p.sparse.refill = ok;
        read(st, true, fx);
    }
}

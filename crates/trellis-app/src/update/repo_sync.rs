//! Fetch or pull across listed repositories: starts at most `MAX_RUNNING` repositories at a
//! time in the runner, records each result, and starts the next. Git never prompts in these;
//! a repository the user said not to trust is skipped without running anything, and one with
//! no decision is skipped by the runner when its config or hooks would run programs. Stop keeps
//! waiting repositories from starting (git already running can't be cancelled, so those
//! finish); Retry runs a failed or stopped repository again.

use crate::effect::{Effect, ReposEffect};
use crate::state::repo_sync::{MAX_RUNNING, RepoSync, RowState, SyncKind, SyncRow, SyncScope, can_retry, nothing_to_sync, targets};
use crate::state::{AppState, Modal, ToastAction};
use std::time::Instant;
use trellis_core::repo_sync::{Outcome, SyncOp};
use trellis_core::settings::RepoTrust;

/// Starts fetching or pulling every repository in `scope`; while a run is going, shows it.
pub(super) fn start(st: &mut AppState, kind: SyncKind, scope: SyncScope, now: Instant, fx: &mut Vec<Effect>) {
    if st.repo_sync.as_ref().is_some_and(RepoSync::running) {
        st.modal = Some(Modal::SyncRepos);
        return;
    }
    let paths = targets(st, scope);
    if paths.is_empty() {
        st.toast(false, nothing_to_sync(kind, scope), now);
        return;
    }
    let rows = paths
        .into_iter()
        .map(|path| {
            let state = match crate::state::trust::recorded(&st.settings, &path) {
                Some(RepoTrust::NotTrusted) => RowState::Done(Outcome::Untrusted),
                _ => RowState::Waiting,
            };
            let name =
                path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| trellis_core::native_path::shown(&path));
            SyncRow { path, name, state }
        })
        .collect();
    let generation = st.repo_sync.as_ref().map_or(1, |s| s.generation + 1);
    st.repo_sync = Some(RepoSync { generation, kind, scope, rows });
    st.modal = Some(Modal::SyncRepos);
    launch(st, fx);
    finish_if_done(st, now);
}

/// Starts waiting repositories until `MAX_RUNNING` run.
fn launch(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(sync) = st.repo_sync.as_mut() else { return };
    let op = match sync.kind {
        SyncKind::Fetch => SyncOp::Fetch { prune: st.settings.auto_prune },
        SyncKind::Pull => SyncOp::Pull,
    };
    let mut running = sync.rows.iter().filter(|r| r.state == RowState::Running).count();
    for (index, row) in sync.rows.iter_mut().enumerate() {
        if running >= MAX_RUNNING {
            break;
        }
        if row.state != RowState::Waiting {
            continue;
        }
        row.state = RowState::Running;
        running += 1;
        let trusted = crate::state::trust::recorded(&st.settings, &row.path) == Some(RepoTrust::Trusted);
        let path = row.path.clone();
        fx.push(Effect::Repos(ReposEffect::Sync { generation: sync.generation, index, path, op, trusted }));
    }
}

/// One repository finished: records how, reloads the active tab when it was that repository and
/// something changed, and starts the next one.
pub(super) fn synced(st: &mut AppState, generation: u64, index: usize, outcome: Outcome, now: Instant, fx: &mut Vec<Effect>) {
    let Some(row) = st.repo_sync.as_mut().filter(|s| s.generation == generation).and_then(|s| s.rows.get_mut(index)) else {
        return;
    };
    if row.state != RowState::Running {
        return;
    }
    let changed = matches!(outcome, Outcome::Updated { .. });
    row.state = RowState::Done(outcome);
    let path = row.path.clone();
    if changed && st.snap.as_ref().is_some_and(|s| st.names_repo(s, &path)) {
        super::refresh::refresh(st, fx);
    }
    launch(st, fx);
    finish_if_done(st, now);
}

/// Stop: no waiting repository starts. Those running finish by themselves.
pub(super) fn stop(st: &mut AppState, now: Instant) {
    let Some(sync) = st.repo_sync.as_mut().filter(|s| s.can_stop()) else { return };
    for row in sync.rows.iter_mut().filter(|r| r.state == RowState::Waiting) {
        row.state = RowState::Stopped;
    }
    finish_if_done(st, now);
}

/// Retry row `index` of run `generation` (failed or stopped): it waits again and starts when
/// fewer than `MAX_RUNNING` run. A repository now recorded as not trusted is skipped instead.
pub(super) fn retry(st: &mut AppState, generation: u64, index: usize, now: Instant, fx: &mut Vec<Effect>) {
    let settings = &st.settings;
    let Some(row) = st.repo_sync.as_mut().filter(|s| s.generation == generation).and_then(|s| s.rows.get_mut(index)) else {
        return;
    };
    if !can_retry(row) {
        return;
    }
    row.state = match crate::state::trust::recorded(settings, &row.path) {
        Some(RepoTrust::NotTrusted) => RowState::Done(Outcome::Untrusted),
        _ => RowState::Waiting,
    };
    launch(st, fx);
    finish_if_done(st, now);
}

/// A run that just ended while its dialog is closed says how it went in a toast that opens it.
fn finish_if_done(st: &mut AppState, now: Instant) {
    let Some(sync) = st.repo_sync.as_ref().filter(|s| !s.running()) else { return };
    if matches!(st.modal, Some(Modal::SyncRepos)) {
        return;
    }
    let ok = !sync.rows.iter().any(|r| matches!(r.state, RowState::Done(Outcome::Failed { .. })));
    let action = ToastAction { label: "Show".into(), msg: Box::new(crate::msg::Msg::Repos(crate::msg::ReposMsg::ShowSync)), workdir: None };
    let text = sync.summary();
    st.toast_action(ok, text, action, now);
}

/// Shows the last run again, if there was one.
pub(super) fn show(st: &mut AppState) {
    if st.repo_sync.is_some() {
        st.modal = Some(Modal::SyncRepos);
    }
}

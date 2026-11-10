//! Results for a tab that is no longer active: what each area keeps (`Msg::on_inactive`).

use crate::msg::{FailedWrite, Inactive, Msg};
use crate::state::AppState;
use crate::state::error_words::typed;
use crate::state::tabs::glance::{CountsRead, Glance};
use std::path::Path;
use std::time::Instant;

/// Records `action` in the undo history in the session of the parked tab for `workdir`.
fn record_parked(st: &mut AppState, workdir: &Path, action: glowfly_core::undo::Action) {
    if let Some(p) = st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()) {
        p.session.undo.record(action);
    }
}

/// Moves the counts the parked tab for `workdir` shows (`Parked::glance`).
fn keep_counts(st: &mut AppState, workdir: &Path, read: CountsRead) {
    let Some(p) = st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()) else { return };
    let mut g = p.glance.take().or_else(|| p.snap.as_ref().map(Glance::of)).unwrap_or_default();
    read.apply(&mut g);
    p.glance = Some(g);
}

/// A result for the repository at `workdir`, whose tab is no longer active: only what its area
/// says still matters (`Msg::on_inactive`) is applied.
pub(super) fn inactive(st: &mut AppState, workdir: &Path, msg: Msg, now: Instant) {
    match msg.on_inactive() {
        Some(Inactive::Log(text)) => st.log_in(Some(workdir), false, text),
        Some(Inactive::Undo(action)) => record_parked(st, workdir, action),
        Some(Inactive::LogAndUndo { text, undo }) => {
            st.log_in(Some(workdir), false, text);
            record_parked(st, workdir, undo);
        }
        Some(Inactive::WorkFile { op, result }) => {
            // A delete or unstage that can be undone goes into that tab's history.
            if let Some(a) = result.as_ref().ok().and_then(|ok| ok.undo()) {
                record_parked(st, workdir, a.clone());
            }
            super::work_files::inactive_result(st, workdir, op, result, now);
        }
        Some(Inactive::WriteFailed { at, error, undo }) => write_failed(st, workdir, at, &error, undo),
        Some(Inactive::IndexBuilt { build, result }) => super::history_index::parked_built(st, workdir, build, result),
        Some(Inactive::ConflictFilesUnread(e)) => {
            let t = st.settings.lang();
            st.log_in(Some(workdir), false, super::conflict_files::read_failed(t, &e));
        }
        Some(Inactive::Counts(read)) => keep_counts(st, workdir, read),
        None => tracing::debug!("dropped a result for {}, which is no longer the active tab", workdir.display()),
    }
}

/// A write for the parked tab at `workdir` failed: its log line, in the user's language, and
/// its undo when it changed files anyway.
fn write_failed(
    st: &mut AppState,
    workdir: &Path,
    at: FailedWrite,
    error: &glowfly_core::error::WriteFailed,
    undo: Option<glowfly_core::undo::Action>,
) {
    let t = st.settings.lang();
    let words = typed::write(t, error);
    let text = match at {
        FailedWrite::LocalOp => words,
        FailedWrite::LocalOpPartial => super::local_done::partial_text(t, error),
        FailedWrite::Revert { path, what } => super::revert::fail_text(&path, what, &words),
        FailedWrite::Restore { path } => crate::msg::restore_failed(&path, &words, undo.is_some(), None),
        // The key hint would name the active tab's Undo, which isn't this one's.
        FailedWrite::DiscardPaths { path, done } => super::work_files_discard::stopped_text(t, done, &path, &words, ""),
        FailedWrite::DeletePaths { path, done } => super::work_files_delete_many::stopped_text(t, done, &path, &words, ""),
    };
    st.log_in(Some(workdir), false, text);
    if let Some(a) = undo {
        record_parked(st, workdir, a);
    }
}

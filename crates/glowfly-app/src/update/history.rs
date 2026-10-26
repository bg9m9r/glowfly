//! File and folder history: opening the commits that changed a file or folder, selecting one
//! (which opens its diff of the file), searching further back, re-running the walk when HEAD
//! moves, and jumping to a commit on the graph. The selected folder file and the File view are
//! in `history_view.rs`.

use super::{history_find, history_view};
use crate::effect::Effect;
use crate::msg::HistoryMsg;
use crate::state::AppState;
use crate::state::history::{FileHistoryView, HistoryKind};
use git2::Oid;
use glowfly_core::repo::{DiffTarget, FileHistory, HistoryStart, Snapshot};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: HistoryMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        HistoryMsg::OpenFileHistory { path, at } => open(st, path, at, None, HistoryKind::File, fx),
        HistoryMsg::OpenFolderHistory { path, at } => open(st, path, at, None, HistoryKind::Folder, fx),
        HistoryMsg::FileHistoryLoaded { path, generation, result } => {
            if generation != st.session.history.generation {
                return; // superseded: closed, reopened, re-run, or parked with its tab
            }
            match result {
                Ok(h) => loaded(st, &path, h, fx),
                Err(e) => {
                    let Some(view) = st.session.history.view.as_mut().filter(|v| v.path == path) else { return };
                    view.loading = false;
                    view.rerun = false;
                    view.error = Some(e.clone());
                    st.log(false, format!("Could not load the history of {path}: {e}"));
                }
            }
        }
        HistoryMsg::SelectHistoryEntry(i) => select(st, i, fx),
        HistoryMsg::SelectHistoryFile(i) => history_view::select_file(st, i, fx),
        HistoryMsg::SetHistoryPane(pane) => history_view::set_pane(st, pane, fx),
        HistoryMsg::SetFilePreview(on) => history_view::set_preview(st, on, fx),
        HistoryMsg::FileTextLoaded { oid, path, before, result } => history_view::text_loaded(st, oid, &path, before, result),
        HistoryMsg::FileFindChanged => history_find::find_changed(st),
        HistoryMsg::FileFindStep(d) => history_find::find_step(st, d),
        HistoryMsg::FocusFileFind => history_find::focus_find(st),
        HistoryMsg::FileGoToLine => history_find::go_to_line(st),
        HistoryMsg::StepHistoryFile(d) => history_view::step_file(st, d, fx),
        HistoryMsg::MoreFileHistory => {
            let Some(view) = st.session.history.view.as_mut().filter(|v| !v.loading && !v.resume.is_empty()) else { return };
            view.loading = true;
            view.error = None;
            let start = HistoryStart::Resume(view.resume.clone());
            request(st, start, st.settings.commit_limit(), fx);
        }
        HistoryMsg::RetryFileHistory => retry(st, fx),
        HistoryMsg::RetryFileText => history_view::retry_text(st, fx),
        HistoryMsg::CloseFileHistory => {
            close(st);
            super::repo::close_diff(st);
        }
        HistoryMsg::ShowInGraph(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                close(st);
                super::repo::select_commit(st, oid, true, fx);
            } else {
                st.toast(false, not_on_graph(oid), now);
            }
        }
    }
}

/// A batch arrived: the first one (or a re-run's) replaces the list and selects an entry; a
/// Search further batch is added below.
fn loaded(st: &mut AppState, path: &str, mut h: FileHistory, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.history.view.as_mut().filter(|v| v.path == path) else { return };
    if !view.entries.is_empty() && !view.rerun {
        // A Search further walk checks the disk under `path`; after a rename forwards the
        // file is at `head_path` today, which only the first batch (or a re-run) checked.
        let on_disk = view.on_disk;
        view.append(h);
        if view.head_path.is_some() {
            view.on_disk = on_disk;
        }
        view.batches += 1;
        return;
    }
    view.as_of = if h.from_head { None } else { view.at };
    match (view.at, h.head_path.take()) {
        // From HEAD: the file now goes by its new name.
        (None, Some(p)) => {
            view.path = p;
            view.head_path = None;
        }
        (_, p) => view.head_path = p,
    }
    let want = view.select.take();
    let keep_file = view.keep_file.take();
    let rerun = std::mem::take(&mut view.rerun);
    view.batches = view.batches.max(1);
    view.replace(h);
    if view.entries.is_empty() {
        view.text = None;
        super::repo::close_diff(st);
        return;
    }
    let i = want.and_then(|oid| view.entries.iter().position(|e| e.oid == oid)).unwrap_or(0);
    if rerun {
        reselect(st, i, keep_file, fx);
    } else {
        select(st, i, fx);
    }
}

/// Opens the history of the file or folder `path` from `at` (HEAD when None), closing the views
/// it replaces; the first batch selects `select` when it lists it, else its first entry.
pub(super) fn open(st: &mut AppState, path: String, at: Option<Oid>, select: Option<Oid>, kind: HistoryKind, fx: &mut Vec<Effect>) {
    if st.workdir().is_none() {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::blame::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    let mut view = FileHistoryView::loading_kind(path, at, kind);
    view.select = select;
    view.pane = st.settings.history.pane;
    st.session.history.view = Some(view);
    request(st, at.map_or(HistoryStart::Head, HistoryStart::At), st.settings.commit_limit(), fx);
}

/// Why commit `oid` can't be selected on the graph.
pub(super) fn not_on_graph(oid: Oid) -> String {
    format!("{} is not visible on the graph: its branch is hidden or it is older than the commit limit", &oid.to_string()[..7])
}

/// Selects entry `i` (its first file, in a folder history) and opens that commit's diff of the
/// file (at its path then).
fn select(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.history.view.as_mut().filter(|v| i < v.entries.len()) else { return };
    view.selected = Some(i);
    view.file = 0;
    let target = view.target();
    st.view.history_scroll_to = Some(i);
    match target {
        Some(t) => super::repo::open_diff(st, t, fx),
        None => super::repo::close_diff(st),
    }
    history_view::want_text(st, fx);
}

/// After a re-run: selects entry `i` again, and `keep_file` in a folder entry when it still
/// lists it, opening the diff only when it isn't the one on screen.
fn reselect(st: &mut AppState, i: usize, keep_file: Option<String>, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.history.view.as_mut().filter(|v| i < v.entries.len()) else { return };
    view.selected = Some(i);
    view.file = keep_file.and_then(|k| view.entries[i].files.iter().position(|f| f.path == k)).unwrap_or(0);
    let has_target = view.target().is_some();
    st.view.history_scroll_to = Some(i);
    if !has_target {
        super::repo::close_diff(st);
    } else if let Some(t) = missing_diff(st) {
        super::repo::open_diff(st, t, fx);
    }
    history_view::want_text(st, fx);
}

/// Where a re-run of `view` starts: at its commit, or at HEAD following a rename since
/// `old_head` (a file at HEAD; folder renames are not followed).
fn start_for(view: &FileHistoryView, old_head: Option<Oid>) -> HistoryStart {
    match (view.at, view.kind) {
        (Some(c), _) => HistoryStart::At(c),
        (None, HistoryKind::File) => old_head.map_or(HistoryStart::Head, HistoryStart::Moved),
        (None, HistoryKind::Folder) => HistoryStart::Head,
    }
}

/// A refresh brought snapshot `new`: when HEAD moved (a commit, checkout, pull, reset, ...)
/// the open history walks again, as deep as it had searched. The list stays until the result
/// replaces it. Called before `st.snap` is replaced.
pub(super) fn head_moved(st: &mut AppState, new: &Snapshot, fx: &mut Vec<Effect>) {
    let Some(old) = st.snap.as_ref().map(|s| s.head_oid).filter(|h| *h != new.head_oid) else { return };
    let base = st.settings.commit_limit();
    let Some(view) = st.session.history.view.as_mut() else { return };
    if let Some(e) = view.selected_entry() {
        view.select = Some(e.oid);
    }
    if view.kind == HistoryKind::Folder
        && let Some((_, p, _)) = view.shown()
    {
        view.keep_file = Some(p.to_owned());
    }
    view.rerun = true;
    view.loading = true;
    view.error = None;
    let start = start_for(view, old);
    let limit = base * view.batches.max(1);
    request(st, start, limit, fx);
}

/// Try again after a failed walk: the first batch (or a re-run) again as deep as it had
/// searched, or the Search further batch that failed.
fn retry(st: &mut AppState, fx: &mut Vec<Effect>) {
    let base = st.settings.commit_limit();
    let Some(view) = st.session.history.view.as_mut().filter(|v| !v.loading && v.error.is_some()) else { return };
    view.loading = true;
    view.error = None;
    let (start, limit) = if view.entries.is_empty() || view.rerun {
        (start_for(view, None), base * view.batches.max(1))
    } else {
        (HistoryStart::Resume(view.resume.clone()), base)
    };
    request(st, start, limit, fx);
}

/// Starts a walk examining at most `limit` commits; results of older requests are ignored from
/// now on.
fn request(st: &mut AppState, start: HistoryStart, limit: usize, fx: &mut Vec<Effect>) {
    st.session.history.generation += 1;
    if let (Some(workdir), Some(view)) = (st.workdir(), &st.session.history.view) {
        fx.push(Effect::LoadFileHistory {
            workdir,
            path: view.path.clone(),
            start,
            limit,
            folder: view.kind == HistoryKind::Folder,
            generation: st.session.history.generation,
        });
    }
}

/// Closes the view (not the diff); a request still in flight must not reopen it.
pub(super) fn close(st: &mut AppState) {
    if st.session.history.view.take().is_some() {
        st.session.history.generation += 1;
    }
}

/// The selected entry's diff (of the selected file, in a folder history), when it is not the
/// diff on screen.
pub(super) fn missing_diff(st: &AppState) -> Option<DiffTarget> {
    let target = st.session.history.view.as_ref()?.target()?;
    st.diff.as_ref().is_none_or(|(t, _)| *t != target).then_some(target)
}

/// After a tab switch: requests dropped while the tab was parked (the walk, the selected
/// entry's diff and the File view's text) are sent again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(target) = missing_diff(st) {
        super::repo::open_diff(st, target, fx);
    }
    history_view::reload_text(st, fx);
    let base = st.settings.commit_limit();
    let Some(view) = st.session.history.view.as_ref().filter(|v| v.loading) else { return };
    let (start, limit) = if view.entries.is_empty() || view.rerun {
        (start_for(view, None), base * view.batches.max(1))
    } else {
        (HistoryStart::Resume(view.resume.clone()), base)
    };
    request(st, start, limit, fx);
}

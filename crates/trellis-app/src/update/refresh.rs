//! Reloading the active tab's repository without piling up work: a refresh first checks
//! whether refs, HEAD, config or worktrees changed and only then walks the history again, a
//! refresh asked for while a load runs waits for it (requests merge into one), and so do status
//! reads.

use crate::effect::{Effect, KnownRepo, LoadKind};
use crate::msg::Msg;
use crate::state::refresh::Depth;
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{FileEntry, Snapshot, SubmoduleInfo};

type Status = (Vec<FileEntry>, Vec<FileEntry>, RepositoryState);

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::RepoUnchanged { generation, result, submodules } => {
            if generation != st.load_gen {
                return; // superseded by a newer load
            }
            st.loading = false;
            // A file list dropped in flight (a tab switch) is asked for again.
            if matches!(st.selection, Selection::Commits(_))
                && let Some(snap) = st.snap.take()
            {
                super::selection::refreshed(st, &snap, fx);
                st.snap = Some(snap);
            }
            apply_status(st, result, submodules, now, fx);
            super::forge::refresh(st, false, false, now, fx);
            load_finished(st, fx);
        }
        Msg::StatusLoaded { generation, result, submodules } => {
            if generation != st.load_gen {
                return;
            }
            st.refresh.status_busy = false;
            if std::mem::take(&mut st.refresh.status_queued) {
                // Already out of date: show it, but leave the diff to the next read.
                if let (Ok(status), Some(snap)) = (result, st.snap.as_mut()) {
                    set_status(snap, status, submodules);
                }
                refresh_status(st, fx);
                return;
            }
            apply_status(st, result, submodules, now, fx);
        }
        _ => {}
    }
}

/// Loads the repository at `path` into the active tab, superseding any load in flight.
pub(super) fn load(st: &mut AppState, path: PathBuf, kind: LoadKind, fx: &mut Vec<Effect>) {
    start(st, path, kind, None, fx);
}

fn start(st: &mut AppState, path: PathBuf, kind: LoadKind, known: Option<KnownRepo>, fx: &mut Vec<Effect>) {
    st.load_gen += 1;
    st.loading = true;
    // Status reads of the older generation are dropped, and the load reads the status anyway.
    st.refresh = Default::default();
    let opts = st.load_options(&path);
    let known = known.filter(|_| st.snap.as_ref().is_some_and(|s| s.loaded_with == opts));
    fx.push(Effect::LoadRepo { path, opts, generation: st.load_gen, kind, known });
}

/// Refreshes the active tab, walking the history again only if refs, HEAD, config, worktrees or
/// the load options changed.
pub(super) fn refresh(st: &mut AppState, fx: &mut Vec<Effect>) {
    request(st, Depth::Checked, fx);
}

/// Reloads the active tab in full (F5). Asked for by the user, it starts at once, superseding a
/// load still running.
pub(super) fn reload(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(path) = st.workdir() {
        start(st, path, LoadKind::Refresh, None, fx);
    }
}

/// A full reload the app needs (the load options changed), after any load still running.
pub(super) fn reload_after(st: &mut AppState, fx: &mut Vec<Effect>) {
    request(st, Depth::Full, fx);
}

fn request(st: &mut AppState, depth: Depth, fx: &mut Vec<Effect>) {
    let Some(path) = st.workdir() else { return };
    if st.loading {
        st.refresh.queued = st.refresh.queued.max(Some(depth));
        return;
    }
    let known = match depth {
        Depth::Checked => st.snap.as_ref().map(|s| KnownRepo { refs_key: s.refs_key, submodules: !s.submodules.is_empty() }),
        Depth::Full => None,
    };
    start(st, path, LoadKind::Refresh, known, fx);
}

/// A load of the current generation ended: runs the refresh asked for meanwhile, if any.
pub(super) fn load_finished(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(depth) = st.refresh.queued.take() {
        request(st, depth, fx);
    }
}

/// Re-reads only the working-tree status (after staging, discarding…). One read runs at a
/// time; asking again meanwhile reads once more when it is done.
pub(super) fn refresh_status(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    if st.refresh.status_busy {
        st.refresh.status_queued = true;
        return;
    }
    st.refresh.status_busy = true;
    let submodules = st.snap.as_ref().is_some_and(|s| !s.submodules.is_empty());
    fx.push(Effect::LoadStatus { workdir, generation: st.load_gen, submodules });
}

fn set_status(snap: &mut Snapshot, status: Status, submodules: Option<Vec<SubmoduleInfo>>) {
    let (unstaged, staged, state) = status;
    let was_dirty = snap.is_dirty();
    snap.unstaged = unstaged;
    snap.staged = staged;
    snap.state = state;
    // A commit or checkout inside a submodule changes its row, not the history.
    if let Some(list) = submodules {
        snap.submodules = list;
    }
    // Clean <-> dirty adds or removes the WIP row above HEAD: lay the graph out again.
    if was_dirty != snap.is_dirty() {
        snap.graph = trellis_core::graph::layout(&snap.commits, snap.head_oid, snap.is_dirty());
    }
}

fn apply_status(
    st: &mut AppState,
    result: Result<Status, String>,
    submodules: Option<Vec<SubmoduleInfo>>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    match result {
        Ok(status) => {
            let Some(snap) = st.snap.as_mut() else { return };
            set_status(snap, status, submodules);
            super::repo::reload_diff(st, fx);
            super::conflict::status_changed(st, now);
        }
        Err(e) => {
            st.log(false, e);
            // Re-read the open diff anyway, so it doesn't stay stale after a hunk operation.
            super::repo::reload_diff(st, fx);
        }
    }
}

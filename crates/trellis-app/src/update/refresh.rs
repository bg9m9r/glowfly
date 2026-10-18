//! Reloading the active tab's repository without piling up work: a refresh first checks
//! whether refs, HEAD, config or worktrees changed and only then walks the history again, a
//! refresh asked for while a load runs waits for it (requests merge into one), and so do status
//! reads.

use crate::effect::{Effect, KnownRepo, LoadKind, StatusRead};
use crate::msg::LoadMsg;
use crate::state::refresh::{Depth, RefreshState, Wanted, can_load_more};
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{ChangeCounts, FileEntry, Snapshot, SubmoduleInfo};

pub(super) type Status = (Vec<FileEntry>, Vec<FileEntry>, RepositoryState);

pub(super) fn handle(st: &mut AppState, msg: LoadMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LoadMsg::RepoUnchanged { generation, result, submodules, open_stamp, slow, lfs_patterns } => {
            if generation != st.load_gen {
                return; // superseded by a newer load
            }
            st.loading = false;
            let state_before = st.snap.as_ref().map(|s| s.state);
            super::notify::unchanged(st);
            // A file list dropped in flight (a tab switch) is asked for again.
            if matches!(st.selection, Selection::Commits(_))
                && let Some(snap) = st.snap.take()
            {
                super::selection::refreshed(st, &snap, fx);
                st.snap = Some(snap);
            }
            apply_status(st, result, submodules, open_stamp, now, fx);
            super::lfs::patterns_read(st, lfs_patterns, fx);
            super::pick::state_changed(st, state_before);
            super::status_tip::check(st, slow.as_ref(), now, fx);
            // A dialog may have been open when the repository loaded.
            super::trust::maybe_ask(st, false);
            super::forge::refresh(st, false, false, now, fx);
            super::push_pr::loaded(st, fx);
            super::worktree_counts::want(st, fx);
            load_finished(st, fx);
        }
        LoadMsg::StatusLoaded { generation, result, submodules, open_stamp, slow, lfs_patterns } => {
            if generation != st.load_gen {
                return;
            }
            st.refresh.status_busy = false;
            // Applied even when the status itself is out of date and deferred.
            super::lfs::patterns_read(st, lfs_patterns, fx);
            let state_before = st.snap.as_ref().map(|s| s.state);
            if let Some(queued) = st.refresh.status_queued.take() {
                // Already out of date: show it, but leave the diff to the next read.
                match super::refresh_paths::show_deferred(st, result, submodules, queued) {
                    Wanted::Whole => refresh_status(st, fx),
                    Wanted::Paths(set) => super::refresh_paths::refresh_paths(st, set.into_iter().collect(), fx),
                }
            } else {
                match result {
                    Ok(StatusRead::Paths(ps)) => super::refresh_paths::apply(st, ps, submodules, open_stamp, now, fx),
                    Ok(StatusRead::Whole(u, s, state)) => apply_status(st, Ok((u, s, state)), submodules, open_stamp, now, fx),
                    Err(e) => apply_status(st, Err(e), submodules, open_stamp, now, fx),
                }
            }
            super::pick::state_changed(st, state_before);
            super::status_tip::check(st, slow.as_ref(), now, fx);
        }
        LoadMsg::SpeedUpStatus(speedups) => super::status_tip::speed_up(st, speedups, fx),
        LoadMsg::LoadMoreCommits if st.snap.as_ref().is_some_and(can_load_more) => more_commits(st, fx),
        LoadMsg::NearHistoryEnd if st.settings.load_more_on_scroll && st.snap.as_ref().is_some_and(can_load_more) && !st.loading_more() => {
            more_commits(st, fx)
        }
        // Everything is loaded already, the preference is off, or a load for more is pending.
        LoadMsg::LoadMoreCommits | LoadMsg::NearHistoryEnd => {}
    }
}

/// Loads another commit limit's worth of history into the active tab (a full walk, since the
/// load options change), after any load still running.
fn more_commits(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.refresh.extra_commits += st.settings.commit_limit();
    refresh(st, fx);
}

/// Loads the repository at `path` into the active tab, superseding any load in flight.
pub(super) fn load(st: &mut AppState, path: PathBuf, kind: LoadKind, fx: &mut Vec<Effect>) {
    start(st, path, kind, None, fx);
}

fn start(st: &mut AppState, path: PathBuf, kind: LoadKind, known: Option<KnownRepo>, fx: &mut Vec<Effect>) {
    st.load_gen += 1;
    st.loading = true;
    // Status reads of the older generation are dropped, and the load reads the status anyway.
    // A background count of other worktrees keeps running: its result checks the refs it was for.
    st.refresh = RefreshState {
        open_stamp: st.refresh.open_stamp,
        extra_commits: st.refresh.extra_commits,
        counting: st.refresh.counting,
        ..Default::default()
    };
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
        Depth::Checked => {
            let git_status = st.runs_git_unasked();
            st.snap.as_ref().map(|s| KnownRepo {
                refs_key: s.refs_key,
                submodules: !s.submodules.is_empty(),
                open: open_file(st),
                git_status,
            })
        }
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
        st.refresh.status_queued = Some(Wanted::queue(st.refresh.status_queued.take(), Wanted::Whole));
        return;
    }
    st.refresh.status_busy = true;
    let submodules = st.snap.as_ref().is_some_and(|s| !s.submodules.is_empty());
    let git_status = st.runs_git_unasked();
    fx.push(Effect::LoadStatus { workdir, generation: st.load_gen, submodules, open: open_file(st), git_status, paths: None });
}

/// The file whose working-tree, index or commit-to-working-tree diff is open.
pub(super) fn open_file(st: &AppState) -> Option<String> {
    st.diff.as_ref().filter(|(t, _)| t.follows_workdir()).map(|(t, _)| t.path().to_owned())
}

/// Whether the diff on screen must be read again after a status read that found the open file
/// at `stamp`. A commit or comparison diff never changes; a diff with the working tree or index
/// on one side only when its file or the index did. Always when a request is outstanding (a hunk operation waits for
/// the reload) or the file history's entry is missing.
pub(super) fn diff_changed(st: &mut AppState, stamp: Option<u64>) -> bool {
    let last = std::mem::replace(&mut st.refresh.open_stamp, stamp);
    if must_reload(st) {
        return true;
    }
    st.diff.as_ref().is_some_and(|(target, _)| target.follows_workdir()) && (stamp.is_none() || stamp != last)
}

/// Whether the diff must be read again whatever its file's stamp: no diff is open, a request is
/// outstanding (a hunk operation waits for the reload), or the file history's entry is missing.
pub(super) fn must_reload(st: &AppState) -> bool {
    st.diff.is_none() || st.diff_gen != st.diff_shown_gen || super::history::missing_diff(st).is_some()
}

pub(super) fn set_status(snap: &mut Snapshot, status: Status, submodules: Option<Vec<SubmoduleInfo>>) {
    let (unstaged, staged, state) = status;
    let was_dirty = snap.is_dirty();
    snap.unstaged = unstaged;
    snap.staged = staged;
    snap.state = state;
    status_set(snap, was_dirty, submodules);
}

/// What follows new status lists in `snap` (dirty before them: `was_dirty`): their counts, the
/// submodules read with them, and the graph's WIP row.
pub(super) fn status_set(snap: &mut Snapshot, was_dirty: bool, submodules: Option<Vec<SubmoduleInfo>>) {
    snap.changes = ChangeCounts::of(&snap.staged, &snap.unstaged);
    // A commit or checkout inside a submodule changes its row, not the history.
    if let Some(list) = submodules {
        snap.submodules = list;
    }
    // Clean <-> dirty adds or removes the WIP row above HEAD: lay the graph out again, keeping
    // the other worktrees' rows with the counts of the last full load or its background count.
    if was_dirty != snap.is_dirty() {
        lay_out_graph(snap);
    }
}

/// Lays `snap`'s graph out again from its commits, its own WIP row and the other worktrees'.
pub(super) fn lay_out_graph(snap: &mut Snapshot) {
    let worktrees = trellis_core::repo::worktree_wip_heads(&snap.worktrees);
    snap.graph = trellis_core::graph::layout(&snap.commits, snap.head_oid, snap.is_dirty(), &worktrees);
}

fn apply_status(
    st: &mut AppState,
    result: Result<Status, String>,
    submodules: Option<Vec<SubmoduleInfo>>,
    open_stamp: Option<u64>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    match result {
        Ok(status) => {
            let Some(snap) = st.snap.as_mut() else { return };
            set_status(snap, status, submodules);
            if diff_changed(st, open_stamp) {
                super::repo::reload_diff(st, fx);
            }
            super::conflict::status_changed(st, now, fx);
            super::compare_wip::status_read(st, fx);
        }
        Err(e) => {
            st.log(false, e);
            // Re-read the open diff anyway, so it doesn't stay stale after a hunk operation.
            super::repo::reload_diff(st, fx);
        }
    }
}

//! Status reads after an index change made outside Glowfly (`WatchMsg::IndexChanged`: a `git add`
//! or `reset` in a terminal, an IDE staging files): the runner compares the index with the memo
//! the lists on screen were read from (`Snapshot::index_memo`) and reads both lists' rows only for
//! the paths whose entries changed, the burst's work-tree paths and the staged rows a rename may
//! pair with (`repo::status_after_index_change`), or the whole status when that can't be done.

use super::refresh::{diff_changed, must_reload, open_file, refresh_status, status_set};
use crate::effect::{Effect, IndexRequest};
use crate::state::AppState;
use crate::state::refresh::Wanted;
use glowfly_core::repo::{self, FileEntry, PathsStatus, Snapshot, SubmoduleInfo};
use glowfly_core::watch::MAX_WATCH_PATHS;
use std::time::Instant;

/// Reads what the index change can have changed, with the work-tree paths `paths`. Without a memo
/// of the index on screen, or past `MAX_WATCH_PATHS`, reads the whole status. One read runs at a
/// time; asked for meanwhile, it runs when that one is done (merged with other reads asked for).
pub(super) fn refresh_index(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Some(old) = st.snap.as_ref().and_then(|s| s.index_memo.clone()).filter(|_| paths.len() <= MAX_WATCH_PATHS) else {
        return refresh_status(st, fx);
    };
    if st.refresh.status_busy {
        let wanted = Wanted::queue(st.refresh.status_queued.take(), Wanted::Index(paths.into_iter().collect()));
        st.refresh.status_queued = Some(wanted);
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let submodules = !snap.submodules.is_empty();
    let partners = partners(&snap.staged);
    st.refresh.status_busy = true;
    let git_status = st.runs_git_unasked();
    let read = IndexRequest { old, partners, paths };
    fx.push(Effect::LoadIndexStatus { workdir, generation: st.load_gen, submodules, open: open_file(st), git_status, read });
}

/// The staged rows a changed path may pair with as a rename: additions, deletions and renames,
/// with each rename's old path; sorted, each once.
fn partners(staged: &[FileEntry]) -> Vec<String> {
    let mut out: Vec<String> = staged
        .iter()
        .filter(|f| matches!(f.status, 'A' | 'D' | 'R'))
        .flat_map(|f| std::iter::once(f.path.clone()).chain(f.renamed_from.clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Merges an index read into `snap`'s lists, as `refresh::set_status` does for a whole read
/// (the counts, the submodules, the graph's WIP row).
pub(super) fn merge_into(snap: &mut Snapshot, ps: PathsStatus, staged: Vec<FileEntry>, submodules: Option<Vec<SubmoduleInfo>>) {
    let was_dirty = snap.is_dirty();
    repo::merge_staged(&mut snap.staged, &ps.covered, staged, snap.ignorecase);
    repo::merge_unstaged(&mut snap.unstaged, &ps.covered, ps.unstaged, snap.ignorecase);
    snap.state = ps.state;
    status_set(snap, was_dirty, submodules);
}

/// Shows an index read. The open diff is read again only when a request is outstanding, or when
/// the read covers its file and the file or the index changed.
pub(super) fn apply(
    st: &mut AppState,
    ps: PathsStatus,
    staged: Vec<FileEntry>,
    submodules: Option<Vec<SubmoduleInfo>>,
    open_stamp: Option<u64>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(snap) = st.snap.as_mut() else { return };
    let fold = snap.ignorecase;
    let touched = st.diff.as_ref().is_some_and(|(t, _)| t.follows_workdir() && repo::covers(&ps.covered, t.path(), fold));
    merge_into(snap, ps, staged, submodules);
    super::line_counts::status_changed(st, fx);
    if must_reload(st) || (touched && diff_changed(st, open_stamp)) {
        super::repo::reload_diff(st, fx);
    }
    super::conflict::status_changed(st, now, fx);
    super::compare_wip::status_read(st, fx);
}

//! Status reads of only the work-tree paths the file watcher reported (`WatchMsg::RepoChanged`
//! with `paths`): only the unstaged rows under them are read again and merged into the lists on
//! screen; the staged list stays, since only an index change (a whole read) can change it.

use super::refresh::{diff_changed, must_reload, open_file, status_set};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::refresh::Wanted;
use std::time::Instant;
use trellis_core::repo::{self, PathsStatus, Snapshot, SubmoduleInfo};

/// Re-reads the unstaged rows under `paths`. One read runs at a time; paths asked for meanwhile
/// are read together when it is done (or the whole status, when a whole read was asked for too).
pub(super) fn refresh_paths(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    if st.refresh.status_busy {
        let wanted = Wanted::queue(st.refresh.status_queued.take(), Wanted::Paths(paths.into_iter().collect()));
        st.refresh.status_queued = Some(wanted);
        return;
    }
    st.refresh.status_busy = true;
    let (submodules, fold) = st.snap.as_ref().map_or((false, false), |s| (!s.submodules.is_empty(), s.ignorecase));
    let git_status = st.runs_git_unasked();
    // The open file's stamp only matters when the read may cover it.
    let open = open_file(st).filter(|p| repo::covers(&paths, p, fold));
    fx.push(Effect::LoadStatus { workdir, generation: st.load_gen, submodules, open, git_status, paths: Some(paths) });
}

/// Merges a read of some paths into `snap`'s unstaged list, as `refresh::set_status` does for a
/// whole read (the counts, the submodules, the graph's WIP row).
pub(super) fn merge_into(snap: &mut Snapshot, ps: PathsStatus, submodules: Option<Vec<SubmoduleInfo>>) {
    let was_dirty = snap.is_dirty();
    repo::merge_unstaged(&mut snap.unstaged, &ps.covered, ps.unstaged, snap.ignorecase);
    snap.state = ps.state;
    status_set(snap, was_dirty, submodules);
}

/// Shows a read of some paths. The open diff is read again only when a request is outstanding,
/// or when the read covers its file and the file changed.
pub(super) fn apply(
    st: &mut AppState,
    ps: PathsStatus,
    submodules: Option<Vec<SubmoduleInfo>>,
    open_stamp: Option<u64>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(snap) = st.snap.as_mut() else { return };
    let fold = snap.ignorecase;
    let touched = st.diff.as_ref().is_some_and(|(t, _)| t.is_working_tree() && repo::covers(&ps.covered, t.path(), fold));
    merge_into(snap, ps, submodules);
    if must_reload(st) || (touched && diff_changed(st, open_stamp)) {
        super::repo::reload_diff(st, fx);
    }
    super::conflict::status_changed(st, now);
}

#[cfg(test)]
#[path = "refresh_paths_tests.rs"]
mod tests;

//! Status reads: the whole status, only the unstaged rows under some work-tree paths the file
//! watcher reported, or, after an index change, both lists' rows of the paths whose index
//! entries changed; the last two fall back to the whole status (in the same effect) when those
//! paths can't be read on their own.

use crate::effect::{IndexRequest, StatusRead};
use crate::msg::{LoadMsg, Msg};
use glowfly_core::repo;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub(super) fn load(
    workdir: &Path,
    generation: u64,
    submodules: bool,
    open: Option<String>,
    git_status: bool,
    paths: Option<Vec<String>>,
) -> Msg {
    let open_stamp = open.and_then(|p| repo::open_file_stamp(workdir, &p));
    if let Some(paths) = paths {
        match repo::status_of_paths(workdir, &paths, git_status) {
            Ok(Some(ps)) => {
                // Only a covered submodule's row can have changed.
                let submodules = if ps.submodule_touched { repo::submodules_if_any(workdir, submodules).ok().flatten() } else { None };
                let result = Ok(StatusRead::Paths(ps));
                return loaded(generation, result, submodules, open_stamp, None);
            }
            Ok(None) => {}
            Err(e) => return loaded(generation, Err(e.to_string()), None, open_stamp, None),
        }
    }
    whole(workdir, generation, submodules, open_stamp, git_status)
}

/// An index read (`Effect::LoadIndexStatus`), or the whole status when it can't be read so.
pub(super) fn load_index(
    workdir: &Path,
    generation: u64,
    submodules: bool,
    open: Option<String>,
    git_status: bool,
    read: IndexRequest,
) -> Msg {
    let open_stamp = open.and_then(|p| repo::open_file_stamp(workdir, &p));
    match repo::status_after_index_change(workdir, &read.old, &read.partners, &read.paths, git_status) {
        Ok(Some(r)) => {
            let submodules = if r.paths.submodule_touched { repo::submodules_if_any(workdir, submodules).ok().flatten() } else { None };
            let result = Ok(StatusRead::Index { paths: r.paths, staged: r.staged });
            loaded(generation, result, submodules, open_stamp, Some(Arc::new(r.memo)))
        }
        Ok(None) => whole(workdir, generation, submodules, open_stamp, git_status),
        Err(e) => loaded(generation, Err(e.to_string()), None, open_stamp, None),
    }
}

/// A read of some paths: no slow note, no LFS patterns.
fn loaded(
    generation: u64,
    result: Result<StatusRead, String>,
    submodules: Option<Vec<repo::SubmoduleInfo>>,
    open_stamp: Option<u64>,
    index: Option<Arc<repo::IndexMemo>>,
) -> Msg {
    Msg::Load(LoadMsg::StatusLoaded { generation, result, submodules, open_stamp, slow: None, lfs_patterns: None, index })
}

fn whole(workdir: &Path, generation: u64, submodules: bool, open_stamp: Option<u64>, git_status: bool) -> Msg {
    let started = Instant::now();
    let (result, index) = match repo::status_and_index(workdir, git_status) {
        Ok((u, s, state, memo)) => (Ok(StatusRead::Whole(u, s, state)), memo.map(Arc::new)),
        Err(e) => (Err(e.to_string()), None),
    };
    let slow = repo::slow(started.elapsed(), workdir);
    let submodules = repo::submodules_if_any(workdir, submodules).ok().flatten();
    let lfs_patterns = Some(glowfly_core::lfs::repo_patterns(workdir));
    Msg::Load(LoadMsg::StatusLoaded { generation, result, submodules, open_stamp, slow, lfs_patterns, index })
}

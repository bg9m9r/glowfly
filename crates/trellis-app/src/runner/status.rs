//! Status reads: the whole status, or only the unstaged rows under some work-tree paths the
//! file watcher reported, falling back to the whole status (in the same effect) when those paths
//! can't be read on their own.

use crate::effect::StatusRead;
use crate::msg::{LoadMsg, Msg};
use std::path::Path;
use std::time::Instant;
use trellis_core::repo;

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
                return Msg::Load(LoadMsg::StatusLoaded { generation, result, submodules, open_stamp, slow: None, lfs_patterns: None });
            }
            Ok(None) => {}
            Err(e) => {
                let result = Err(e.to_string());
                let lfs_patterns = None;
                return Msg::Load(LoadMsg::StatusLoaded { generation, result, submodules: None, open_stamp, slow: None, lfs_patterns });
            }
        }
    }
    let started = Instant::now();
    let result = repo::status_at(workdir, git_status).map(|(u, s, state)| StatusRead::Whole(u, s, state)).map_err(|e| e.to_string());
    let slow = repo::slow(started.elapsed(), workdir);
    let submodules = repo::submodules_if_any(workdir, submodules).ok().flatten();
    let lfs_patterns = Some(trellis_core::lfs::root_patterns(workdir));
    Msg::Load(LoadMsg::StatusLoaded { generation, result, submodules, open_stamp, slow, lfs_patterns })
}

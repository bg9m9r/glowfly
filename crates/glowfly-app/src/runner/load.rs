//! Loading the active tab's repository (`Effect::LoadRepo`): only the status when the refs are
//! as the snapshot on screen has them, the next commits of its walk for a load for more
//! (`repo::load_more`), else everything, taking the commits on screen instead of reading them
//! again (`repo::load_reusing`).

use super::execute::err;
use super::git_env;
use crate::effect::{KnownRepo, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg};
use glowfly_core::repo::{self, GraphCache, LoadOptions, MoreLoaded, Prior};
use std::path::PathBuf;

/// Runs one repository load and returns its result message.
pub(super) fn load_repo(
    path: PathBuf,
    opts: LoadOptions,
    generation: u64,
    kind: LoadKind,
    known: Option<KnownRepo>,
    prior: Option<Prior>,
    graph_cache: &GraphCache,
) -> Msg {
    let unchanged = known.as_ref().is_some_and(|k| repo::refs_key(&path).is_ok_and(|key| key == k.refs_key));
    if kind == LoadKind::More
        && unchanged
        && let Some(p) = &prior
    {
        match repo::load_more(&path, p, opts.max_commits) {
            Ok(MoreLoaded::Page(page)) => return Msg::Load(LoadMsg::HistoryExtended { generation, result: Ok(page) }),
            Err(e) => return Msg::Load(LoadMsg::HistoryExtended { generation, result: Err(err(e)) }),
            // A new commit would go above a loaded one: walk it all.
            Ok(MoreLoaded::NeedsFullLoad) => {}
        }
    } else if kind != LoadKind::More
        && unchanged
        && let Some(k) = known
    {
        let open_stamp = k.open.and_then(|p| repo::open_file_stamp(&path, &p));
        let started = std::time::Instant::now();
        let read = repo::status_and_index(&path, k.git_status).map_err(err);
        let slow = repo::slow(started.elapsed(), &path);
        let submodules = repo::submodules_if_any(&path, k.submodules).ok().flatten();
        let lfs_patterns = Some(glowfly_core::lfs::root_patterns(&path));
        let (result, index) = match read {
            Ok((u, s, state, memo)) => (Ok((u, s, state)), memo.map(std::sync::Arc::new)),
            Err(e) => (Err(e), None),
        };
        return Msg::Load(LoadMsg::RepoUnchanged { generation, result, submodules, open_stamp, slow, lfs_patterns, index });
    }
    // A load for more that can't go on from the walk is a refresh at the larger count.
    let kind = if kind == LoadKind::More { LoadKind::Refresh } else { kind };
    let result = match repo::load_reusing(&path, &opts, graph_cache, prior.as_ref()) {
        Err(glowfly_core::error::Error::NotOwned(root)) => return git_env::not_owned(generation, kind, root),
        r => r.map(Box::new).map_err(|e| format!("Could not open {}: {e}", glowfly_core::native_path::shown(&path))),
    };
    Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result })
}

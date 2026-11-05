//! The `Execute` impl of `LoadEffect`, and loading the active tab's repository
//! (`LoadEffect::Repo`): only the status when the refs are as the snapshot on screen has them,
//! the next commits of its walk for a load for more (`repo::load_more`), else everything, taking
//! the commits on screen instead of reading them again (`repo::load_reusing`). The status reads
//! are in status.rs.

use super::execute::{Cx, Execute};
use super::git_env;
use super::status;
use glowfly_core::error::ReadFailed;
use glowfly_core::repo::{self, GraphCache, LoadOptions, MoreLoaded, Prior};
use glowfly_model::effect::{KnownRepo, LoadEffect, LoadKind};
use glowfly_model::msg::{LoadMsg, Msg, RepoMsg};
use std::path::PathBuf;

impl Execute for LoadEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            LoadEffect::Repo { path, opts, generation, kind, known, prior } => {
                load_repo(path, opts, generation, kind, known, prior, cx.graph_cache)
            }
            LoadEffect::Status { workdir, generation, submodules, open, git_status, paths } => {
                status::load(&workdir, generation, submodules, open, git_status, paths, cx.graph_cache)
            }
            LoadEffect::IndexStatus { workdir, generation, submodules, open, git_status, read } => {
                status::load_index(&workdir, generation, submodules, open, git_status, read, cx.graph_cache)
            }
        })
    }
}

/// Runs one repository load and returns its result message.
fn load_repo(
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
            Err(e) => return Msg::Load(LoadMsg::HistoryExtended { generation, result: Err(ReadFailed::from(e)) }),
            // A new commit would go above a loaded one: walk it all.
            Ok(MoreLoaded::NeedsFullLoad) => {}
        }
    } else if kind != LoadKind::More
        && unchanged
        && let Some(k) = known
    {
        let open_stamp = k.open.and_then(|p| repo::open_file_stamp(&path, &p));
        let started = std::time::Instant::now();
        let read = repo::status_and_index_with(&path, k.git_status, Some(graph_cache.records())).map_err(ReadFailed::from);
        let slow = repo::slow(started.elapsed(), &path);
        let rows = read.as_ref().ok().map(|(u, ..)| u.as_slice());
        let submodules = repo::submodules_if_any(&path, k.submodules, rows, Some(graph_cache.submodules())).ok().flatten();
        let lfs_patterns = Some(glowfly_core::lfs::repo_patterns(&path));
        let (result, index) = match read {
            Ok((u, s, state, memo)) => (Ok((u, s, state)), memo.map(std::sync::Arc::new)),
            Err(e) => (Err(e), None),
        };
        return Msg::Load(LoadMsg::RepoUnchanged { generation, result, submodules, open_stamp, slow, lfs_patterns, index });
    }
    // A load for more that can't go on from the walk is a refresh at the larger count.
    let kind = if kind == LoadKind::More { LoadKind::Refresh } else { kind };
    let result = match repo::load_reusing(&path, &opts, graph_cache, prior.as_ref()) {
        Err(glowfly_core::error::Error::NotOwned(refusal)) => return git_env::not_owned(generation, kind, refusal),
        r => r
            .map(Box::new)
            .map_err(|e| ReadFailed::OpenFailed { path: glowfly_core::native_path::shown(&path), why: Box::new(ReadFailed::from(e)) }),
    };
    Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::error::ReadFailed;

    #[test]
    fn a_folder_with_no_repository_fails_to_open_naming_it() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-load-plain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let msg = load_repo(dir.clone(), LoadOptions::default(), 1, LoadKind::Open, None, None, &GraphCache::default());
        let _ = std::fs::remove_dir_all(&dir);
        let Msg::Repo(RepoMsg::RepoLoaded { result: Err(e), .. }) = msg else { panic!("a failed load") };
        assert!(e.to_string().starts_with("Could not open "), "{e}");
        let ReadFailed::OpenFailed { path, why } = e else { panic!("an open failure: {e:?}") };
        assert_eq!(path, glowfly_core::native_path::shown(&dir));
        match *why {
            ReadFailed::NotARepository { .. } => {}
            ReadFailed::Said { said } => assert!(!said.is_empty()),
            other => panic!("not a repository, or libgit2's words: {other:?}"),
        }
    }
}

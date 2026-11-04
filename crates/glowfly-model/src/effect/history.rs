//! History and search work (`Effect::History`, runner/history.rs): the walk of a file's or
//! folder's commits, the path search over the commits on screen and their line stats; and the
//! file history's request for a file's text (`Effect::LoadFileText`). Scoped and read-only.

use super::EffectMeta;
use super::SandboxRule;
use git2::Oid;
use glowfly_core::repo::HistoryStart;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum HistoryEffect {
    /// The commits that changed `path` (anything under it with `folder`), examining at most `limit`.
    FileHistory { workdir: PathBuf, path: String, start: HistoryStart, limit: usize, folder: bool, generation: u64 },
    /// Which of `oids` (newest first) changed a path matching `terms`, for search request
    /// `request`; reported in chunks as SearchMsg::PathsSearched. Handled by Runner::run's
    /// path-search worker: a newer request replaces one waiting and stops one running.
    SearchPaths { workdir: PathBuf, request: u64, terms: Vec<glowfly_core::path_query::PathTerm>, oids: Vec<Oid> },
    /// The line counts of `oids` for the graph's CHANGES column; handled by Runner::run's stats
    /// worker, newest request first.
    Stats { workdir: PathBuf, oids: Vec<Oid> },
}

impl EffectMeta for HistoryEffect {
    fn name(&self) -> &'static str {
        match self {
            HistoryEffect::FileHistory { .. } => "file-history",
            HistoryEffect::SearchPaths { .. } => "search-paths",
            HistoryEffect::Stats { .. } => "stats",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            HistoryEffect::FileHistory { workdir, .. }
            | HistoryEffect::SearchPaths { workdir, .. }
            | HistoryEffect::Stats { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Reads only.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

/// Read `path` of `workdir` as of commit `oid`.
#[derive(Debug)]
pub struct FileTextRequest {
    pub workdir: PathBuf,
    pub oid: Oid,
    pub path: String,
    /// Read the version before `oid` (which deletes the file).
    pub before: bool,
    /// Give a Git LFS pointer's content; set only when git may run in the repository unasked.
    pub smudge: bool,
}

impl EffectMeta for FileTextRequest {
    fn name(&self) -> &'static str {
        "file-text"
    }

    fn scope(&self) -> Option<&PathBuf> {
        Some(&self.workdir)
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

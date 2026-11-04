//! Loading the active tab's repository and reading its status (`Effect::Load`, runner/load.rs):
//! the whole repository, the working tree's and index's status, and an index change's rows.
//! Reads only.

use super::EffectMeta;
use super::SandboxRule;
use super::{IndexRequest, KnownRepo, LoadKind};
use glowfly_core::repo::LoadOptions;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LoadEffect {
    /// Opens or refreshes the repository at `path`. Unscoped: the load says which tab it is for.
    Repo {
        path: PathBuf,
        opts: LoadOptions,
        generation: u64,
        kind: LoadKind,
        /// The snapshot on screen: when its `refs_key` still matches, only the status is read (`LoadMsg::RepoUnchanged`).
        known: Option<KnownRepo>,
        /// The history on screen: commits a full load takes instead of reading them again, and for `More` the walk's frontier.
        prior: Option<glowfly_core::repo::Prior>,
    },
    /// The working tree's and the index's status.
    Status {
        workdir: PathBuf,
        generation: u64,
        /// The last load found submodules: read them again even without a .gitmodules.
        submodules: bool,
        /// The file whose working-tree or index diff is open, for `LoadMsg::StatusLoaded::open_stamp`.
        open: Option<String>,
        /// Let git read the status (fsmonitor, untracked cache); false while the repository isn't trusted.
        git_status: bool,
        /// Re-read only these work-tree paths' unstaged rows; None reads everything.
        paths: Option<Vec<String>>,
    },
    /// After an index change: both lists' rows of the changed paths, else the whole status.
    IndexStatus { workdir: PathBuf, generation: u64, submodules: bool, open: Option<String>, git_status: bool, read: IndexRequest },
}

impl EffectMeta for LoadEffect {
    fn name(&self) -> &'static str {
        match self {
            LoadEffect::Repo { .. } => "load-repo",
            LoadEffect::Status { .. } => "load-status",
            LoadEffect::IndexStatus { .. } => "load-index-status",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            LoadEffect::Repo { .. } => None,
            LoadEffect::Status { workdir, .. } | LoadEffect::IndexStatus { workdir, .. } => Some(workdir),
        }
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

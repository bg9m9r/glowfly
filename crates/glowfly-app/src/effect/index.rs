//! The whole-history search index (runner/index.rs): building or updating a tab's index, asking
//! it for a query's matches, and forgetting a tab's build. Reads only (libgit2, no program runs),
//! scoped to the repository's tab.

use glowfly_core::commit_index::{CommitIndex, IndexKey};
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::repo::CommitInfo;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub enum IndexEffect {
    /// Indexes the history walked from `key`, reusing `base` (the tab's previous index) where
    /// it can. Reports `SearchMsg::IndexProgress` now and then and `SearchMsg::IndexBuilt` at the
    /// end, both with `build`; nothing when a newer build for the same repository or a Forget
    /// stopped it. One build runs at a time across tabs, the newest request first.
    Build { workdir: PathBuf, build: u64, key: IndexKey, base: Option<Arc<CommitIndex>> },
    /// The walk positions of `index`'s commits matching `query` (no path terms), reported as
    /// `SearchMsg::IndexQueried` with `request`; a newer query replaces one still waiting.
    Query { workdir: PathBuf, request: u64, index: Arc<CommitIndex>, query: CommitQuery },
    /// The rows of the loaded `commits` matching `query` (no path terms), for a history too
    /// large to match on the UI thread; reported as `SearchMsg::LoadedMatched` with `request`. A
    /// newer one replaces one still waiting, as for Query (on a worker of its own).
    MatchLoaded { workdir: PathBuf, request: u64, commits: Shared, query: CommitQuery },
    /// The tab closed, opened another repository or turned indexing off: stops its build, waiting
    /// or running. Reports nothing.
    Forget { workdir: PathBuf },
}

/// A loaded history shared with a worker; its `Debug` gives only the length (effects are logged).
#[derive(Clone)]
pub struct Shared(pub Arc<[CommitInfo]>);

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} commits", self.0.len())
    }
}

impl IndexEffect {
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            IndexEffect::Build { workdir, .. }
            | IndexEffect::Query { workdir, .. }
            | IndexEffect::MatchLoaded { workdir, .. }
            | IndexEffect::Forget { workdir } => Some(workdir),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            IndexEffect::Build { .. } => "index-build",
            IndexEffect::Query { .. } => "index-query",
            IndexEffect::MatchLoaded { .. } => "match-loaded",
            IndexEffect::Forget { .. } => "index-forget",
        }
    }

    /// Whether the runner times it for the performance log.
    pub fn timed(&self) -> bool {
        match self {
            IndexEffect::Build { .. } | IndexEffect::Query { .. } | IndexEffect::MatchLoaded { .. } | IndexEffect::Forget { .. } => true,
        }
    }
}

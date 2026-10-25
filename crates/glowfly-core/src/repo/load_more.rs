//! Loading more of a history cut short without reading the rest again: the walk goes on from
//! where it stopped (`Snapshot::frontier`) for the next commits only, and the snapshot takes
//! them in below its rows (`Snapshot::extend_history`), laid out from where its lanes stood. The
//! result is what a full load of the larger count gives, as long as the refs are as they were
//! (the caller checks `refs_key`) and no new commit is a child of a loaded one.

use super::walk_reuse::{Frontier, Prior};
use super::{CommitInfo, Snapshot};
use crate::error::Result;
use crate::repo_pool::{self, Use};
use std::path::Path;
use std::sync::Arc;

/// What a load for more found.
pub enum MoreLoaded {
    /// The next commits, for `Snapshot::extend_history`.
    Page(Box<HistoryPage>),
    /// The walk can't go on from where it stopped (no frontier, nothing more asked for, or a new
    /// commit would have to go above a loaded one): load the history in full instead.
    NeedsFullLoad,
}

/// The commits a load for more read, with the history they go on from.
pub struct HistoryPage {
    /// The commits the page goes on from: the snapshot it may be applied to holds these.
    base: Arc<[CommitInfo]>,
    /// The base's commits followed by the new ones.
    commits: Arc<[CommitInfo]>,
    truncated: bool,
    frontier: Option<Arc<Frontier>>,
    max_commits: usize,
}

impl HistoryPage {
    /// How many commits it adds.
    pub fn added(&self) -> usize {
        self.commits.len() - self.base.len()
    }
}

impl std::fmt::Debug for HistoryPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HistoryPage({} + {} commits, truncated: {})", self.base.len(), self.added(), self.truncated)
    }
}

/// The commits after `prior`'s, up to `max_commits` in all, read by going on from where its
/// walk stopped in the repository at `path`.
pub fn load_more(path: &Path, prior: &Prior, max_commits: usize) -> Result<MoreLoaded> {
    let _span = tracing::debug_span!("load_more", path = %path.display(), max = max_commits).entered();
    let Some(frontier) = prior.frontier.as_deref() else { return Ok(MoreLoaded::NeedsFullLoad) };
    if max_commits <= prior.commits.len() {
        return Ok(MoreLoaded::NeedsFullLoad);
    }
    let repo = repo_pool::lend(path, Use::Refs)?;
    let walked = tracing::debug_span!("load_more.walk")
        .in_scope(|| super::walk::more_commits(&repo, &prior.commits, frontier, max_commits - prior.commits.len()))?;
    drop(repo);
    let Some(walked) = walked else { return Ok(MoreLoaded::NeedsFullLoad) };
    let commits: Arc<[CommitInfo]> =
        tracing::debug_span!("load_more.copy").in_scope(|| prior.commits.iter().cloned().chain(walked.commits).collect());
    Ok(MoreLoaded::Page(Box::new(HistoryPage {
        base: Arc::clone(&prior.commits),
        commits,
        truncated: walked.truncated,
        frontier: walked.frontier.map(Arc::new),
        max_commits,
    })))
}

impl Snapshot {
    /// Takes in `page` below the loaded history: its commits, their rows and the graph laid out
    /// for them, as a full load of the larger count would have it. Returns the commit list it
    /// replaces (to be freed off the UI thread for a large history), or the page back when it
    /// doesn't go on from this snapshot's history.
    pub fn extend_history(&mut self, page: HistoryPage) -> std::result::Result<Arc<[CommitInfo]>, HistoryPage> {
        if !Arc::ptr_eq(&page.base, &self.commits) {
            return Err(page);
        }
        let from = self.commits.len();
        let old = std::mem::replace(&mut self.commits, page.commits);
        drop(page.base);
        self.row_of.reserve(self.commits.len() - from);
        for (i, c) in self.commits.iter().enumerate().skip(from) {
            self.row_of.insert(c.oid, i);
        }
        self.truncated = page.truncated;
        self.frontier = page.frontier;
        self.loaded_with.max_commits = page.max_commits;
        self.extend_graph(from);
        Ok(old)
    }
}

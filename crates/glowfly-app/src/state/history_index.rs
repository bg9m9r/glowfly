//! A tab's whole-history search index (`glowfly_core::commit_index`): the index ready for its
//! history, the build running for it, and the index's answer for the current query. Changed by
//! update/history_index.rs (building) and update/search_index.rs (asking it); parked with the tab
//! inside `CommitSearch`.

use crate::state::AppState;
use git2::Oid;
use glowfly_core::commit_index::{CommitIndex, IndexKey};
use glowfly_core::commit_query::CommitQuery;
use std::path::PathBuf;
use std::sync::Arc;

/// A build in flight.
#[derive(Debug)]
pub struct Building {
    /// The build's id (`HistoryIndex::builds` when it was asked for).
    pub build: u64,
    /// The history it indexes.
    pub key: IndexKey,
    /// How far it got, and about how many commits there are.
    pub indexed: usize,
    pub estimate: Option<usize>,
    /// It starts from nothing (no usable earlier index), so the status bar shows it; an update
    /// of an earlier index runs silently.
    pub fresh: bool,
}

/// The index's matches for a query.
pub struct IndexAnswer {
    pub query: CommitQuery,
    /// The index asked.
    pub index: Arc<CommitIndex>,
    /// The matches' walk positions, ascending.
    pub positions: Vec<u32>,
}

/// One tab's index and what it answered.
#[derive(Default)]
pub struct HistoryIndex {
    /// The repository `ready` and `building` are for.
    pub workdir: Option<PathBuf>,
    /// The newest finished index; it may be for an older history until the next build lands.
    pub ready: Option<Arc<CommitIndex>>,
    pub building: Option<Building>,
    /// Counts builds asked for; never reset, so a stale report can't pass for a newer build.
    pub builds: u64,
    /// The history whose build failed: not retried until the history changes.
    pub failed: Option<IndexKey>,
    pub answer: Option<IndexAnswer>,
    /// The query request still answering, if any, and the query and index it asked.
    pub pending: Option<u64>,
    pub asked: Option<(CommitQuery, Arc<CommitIndex>)>,
    /// Counts query requests; never reset.
    pub requests: u64,
    /// An older match Enter asked to load: selected once a load shows it.
    pub jump: Option<Oid>,
}

impl HistoryIndex {
    /// Nothing built or asked, keeping the counters.
    pub fn reset(&mut self) {
        *self = HistoryIndex { builds: self.builds, requests: self.requests, ..Default::default() };
    }

    /// No answer, no query in flight and no match to jump to (the index stays).
    pub fn drop_answer(&mut self) {
        self.answer = None;
        self.pending = None;
        self.asked = None;
        self.jump = None;
    }

    /// Whether it holds an index or a build for one.
    pub fn holds_any(&self) -> bool {
        self.ready.is_some() || self.building.is_some()
    }
}

/// What the search bar's count can say about the whole history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HistoryCount<'a> {
    /// Only the loaded commits are known: today's texts.
    Loaded,
    /// A fresh index is being built.
    Indexing,
    /// The index is being asked for this query.
    Searching,
    /// The index answered: every match's walk position, the commits it holds, and whether that
    /// is the whole history.
    Whole { positions: &'a [u32], commits: usize, complete: bool },
}

impl AppState {
    /// The tab's index when it answers for the loaded history: the graph is cut short, the
    /// preference is on, the index is for this history and the query has no path terms.
    pub fn current_index(&self) -> Option<&Arc<CommitIndex>> {
        let snap = self.snap.as_ref()?;
        let ix = self.search.index.ready.as_ref()?;
        let usable = snap.truncated && self.settings.index_history && !self.search.query.has_paths() && ix.key().covers(snap);
        usable.then_some(ix)
    }

    /// The index's answer for the current query, from the current index.
    pub fn index_answer(&self) -> Option<&IndexAnswer> {
        let ix = self.current_index()?;
        let a = self.search.index.answer.as_ref()?;
        (self.search.active() && a.query == self.search.query && Arc::ptr_eq(&a.index, ix)).then_some(a)
    }

    /// What the search bar's count says beyond the loaded matches.
    pub fn history_count(&self) -> HistoryCount<'_> {
        if let Some(a) = self.index_answer() {
            return HistoryCount::Whole { positions: &a.positions, commits: a.index.len(), complete: a.index.complete() };
        }
        if !self.search.active() || self.search.query.has_paths() {
            return HistoryCount::Loaded;
        }
        if self.current_index().is_some() && self.search.index.pending.is_some() {
            return HistoryCount::Searching;
        }
        let truncated = self.snap.as_ref().is_some_and(|s| s.truncated);
        if truncated && self.indexing().is_some() {
            return HistoryCount::Indexing;
        }
        HistoryCount::Loaded
    }

    /// A fresh build's progress (commits indexed, about how many there are), for the status bar.
    pub fn indexing(&self) -> Option<(usize, Option<usize>)> {
        let b = self.search.index.building.as_ref().filter(|b| b.fresh)?;
        self.settings.index_history.then_some((b.indexed, b.estimate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::CommitInfo;
    use glowfly_core::settings::Settings;
    use std::time::Instant;

    fn info(n: u8) -> CommitInfo {
        let oid = Oid::from_bytes(&[n; 20]).unwrap();
        CommitInfo {
            oid,
            summary: format!("fix {n}").into(),
            body: Default::default(),
            author: glowfly_core::repo::Author::new("A", "a@x.org"),
            time: 0,
            parents: Default::default(),
        }
    }

    #[test]
    fn the_count_follows_the_build_and_the_answer() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let mut snap = crate::state::op_fixture::snapshot("history-count");
        let dir = snap.workdir.clone();
        snap.truncated = true;
        let key = IndexKey::of(&snap);
        st.snap = Some(snap);
        st.search.query = CommitQuery::parse("fix");
        assert_eq!(st.history_count(), HistoryCount::Loaded);

        st.search.index.building = Some(Building { build: 1, key: key.clone(), indexed: 5, estimate: Some(9), fresh: true });
        assert_eq!(st.indexing(), Some((5, Some(9))));
        assert_eq!(st.history_count(), HistoryCount::Indexing);
        st.search.index.building.as_mut().unwrap().fresh = false;
        assert_eq!((st.indexing(), st.history_count()), (None, HistoryCount::Loaded), "an update is silent");
        st.search.index.building = None;

        let ix = Arc::new(CommitIndex::from_commits(key, &[info(1), info(2)]));
        st.search.index.ready = Some(Arc::clone(&ix));
        st.search.index.pending = Some(1);
        assert_eq!(st.history_count(), HistoryCount::Searching);
        st.search.index.answer = Some(IndexAnswer { query: st.search.query.clone(), index: Arc::clone(&ix), positions: vec![0, 1] });
        assert_eq!(st.history_count(), HistoryCount::Whole { positions: &[0, 1], commits: 2, complete: true });
        // Another query, path terms, the preference off or a whole graph leave the answer unused.
        st.search.query = CommitQuery::parse("fix path:a");
        assert_eq!(st.history_count(), HistoryCount::Loaded);
        st.search.query = CommitQuery::parse("fix");
        st.settings.index_history = false;
        assert!(st.current_index().is_none());
        st.settings.index_history = true;
        st.snap.as_mut().unwrap().truncated = false;
        assert!(st.index_answer().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! The graph's commit search: the parsed query, its matches, the filter flag, the author list
//! and the answers for its path terms. Which rows the graph lays out, filtered or not, is
//! `graph_rows::GraphRows`.

use crate::state::history_index::{HistoryCount, HistoryIndex};
use git2::Oid;
use glowfly_core::commit_query::{AuthorCount, CommitQuery};
use glowfly_core::path_query::PathTerm;
use std::collections::HashSet;
use std::time::Instant;

/// The active tab's commit search (parked with the tab). Changed only by update; the view edits
/// the query's text buffer (`ViewState::search`) and sends `SearchMsg`s.
#[derive(Default)]
pub struct CommitSearch {
    /// Show only the matching commits (while the search bar is shown).
    pub filter: bool,
    /// `ViewState::search`, parsed when it last changed.
    pub query: CommitQuery,
    /// Indexes into `snap.commits` of the commits matching `query`, ascending.
    pub matches: Vec<usize>,
    /// The commit index Enter last landed on; the next Enter goes to the first match after it.
    pub cursor: usize,
    /// Enter has landed on a match since the query last changed (so past the last loaded match it
    /// goes on to older ones rather than to the newest).
    pub stepped: bool,
    /// The loaded history's distinct authors for the author picker, worked out when it first
    /// opens and dropped whenever the history changes.
    pub authors: Option<Vec<AuthorCount>>,
    /// Which loaded commits touch the query's path terms, as far as found.
    pub paths: PathSearch,
    /// The whole-history index of a large repository and its answer for the query (parked with
    /// the tab).
    pub index: HistoryIndex,
    /// Matching a large loaded history off the UI thread (update/search_loaded.rs).
    pub loaded: LoadedMatch,
}

/// A typed query waiting for a pause in typing, and the match asked of the query worker.
#[derive(Default)]
pub struct LoadedMatch {
    /// When the typed query is matched, if it is waiting for the pause.
    pub due: Option<Instant>,
    /// The request whose answer is wanted, if any.
    pub pending: Option<u64>,
    /// Counts requests, so an older answer can't pass for a newer request.
    pub requests: u64,
}

/// Answers for the query's path terms, found off the UI thread (update/search_paths.rs). A
/// commit's changed paths never change, so the answers stay valid by oid across reloads.
#[derive(Default)]
pub struct PathSearch {
    /// The terms these answers are for.
    pub terms: Vec<PathTerm>,
    /// The commits looked at.
    pub searched: HashSet<Oid>,
    /// Those of them that touch a term.
    pub touching: HashSet<Oid>,
    /// The request still reporting, if any.
    pub pending: Option<u64>,
    /// Counts requests; never reset, so a stale report can't pass for a newer request.
    pub requests: u64,
}

impl CommitSearch {
    /// Whether there is something to search for.
    pub fn active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Whether the changed files of loaded commits are still being looked through for the
    /// query's path terms.
    pub fn searching_files(&self) -> bool {
        self.query.has_paths() && self.paths.pending.is_some()
    }

    /// Where commit `commit_index` is among the matches (0-based), if it matches.
    pub fn position(&self, commit_index: usize) -> Option<usize> {
        self.matches.binary_search(&commit_index).ok()
    }

    /// The match count for the search bar ("No matches", "2 of 5", "5 matches", "1 match", or
    /// while path terms are searched "Searching files…" and "3 so far, searching…"); `selected`
    /// is the selected commit's index. `history` is what is known beyond the loaded commits
    /// (`AppState::history_count`): an index being built or asked, or its whole-history totals.
    /// `None` when there is no search.
    pub fn count_text(&self, selected: Option<usize>, history: HistoryCount) -> Option<String> {
        if !self.active() {
            return None;
        }
        let m = self.matches.len();
        if self.searching_files() {
            return Some(if m == 0 { "Searching files…".to_owned() } else { format!("{m} so far, searching…") });
        }
        let total = match history {
            HistoryCount::Loaded => m,
            HistoryCount::Indexing => {
                return Some(if m == 0 { "Indexing history…".to_owned() } else { format!("{m} so far, indexing…") });
            }
            HistoryCount::Searching => {
                return Some(if m == 0 { "Searching history…".to_owned() } else { format!("{m} so far, searching…") });
            }
            HistoryCount::Whole { positions, .. } => positions.len(),
        };
        Some(match (total, selected.and_then(|i| self.position(i))) {
            (0, _) => "No matches".to_owned(),
            (_, Some(n)) => format!("{} of {total}", n + 1),
            (1, None) => "1 match".to_owned(),
            (_, None) => format!("{total} matches"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(matches: Vec<usize>, filter: bool) -> CommitSearch {
        CommitSearch { filter, query: CommitQuery::parse("commit"), matches, ..Default::default() }
    }

    #[test]
    fn count_text_describes_the_matches() {
        assert_eq!(CommitSearch::default().count_text(Some(0), HistoryCount::Loaded), None);
        assert_eq!(search(vec![], false).count_text(None, HistoryCount::Loaded).as_deref(), Some("No matches"));
        assert_eq!(search(vec![2], false).count_text(None, HistoryCount::Loaded).as_deref(), Some("1 match"));
        assert_eq!(search(vec![2], false).count_text(Some(2), HistoryCount::Loaded).as_deref(), Some("1 of 1"));
        assert_eq!(search(vec![0, 2, 5], false).count_text(Some(1), HistoryCount::Loaded).as_deref(), Some("3 matches"));
        assert_eq!(search(vec![0, 2, 5], false).count_text(Some(2), HistoryCount::Loaded).as_deref(), Some("2 of 3"));
    }

    #[test]
    fn count_text_with_the_whole_history() {
        let whole = |positions: &'static [u32]| HistoryCount::Whole { positions, commits: 5000, complete: true };
        let s = search(vec![], false);
        assert_eq!(s.count_text(None, HistoryCount::Indexing).as_deref(), Some("Indexing history…"));
        assert_eq!(s.count_text(None, HistoryCount::Searching).as_deref(), Some("Searching history…"));
        assert_eq!(s.count_text(None, whole(&[])).as_deref(), Some("No matches"));
        assert_eq!(s.count_text(None, whole(&[900])).as_deref(), Some("1 match"));
        let s = search(vec![0, 2, 5], false);
        assert_eq!(s.count_text(None, HistoryCount::Indexing).as_deref(), Some("3 so far, indexing…"));
        assert_eq!(s.count_text(None, HistoryCount::Searching).as_deref(), Some("3 so far, searching…"));
        let many: &'static [u32] = Vec::leak((0..1234).collect());
        assert_eq!(s.count_text(Some(5), whole(many)).as_deref(), Some("3 of 1234"));
        assert_eq!(s.count_text(None, whole(many)).as_deref(), Some("1234 matches"));
        // Path terms still being searched win.
        let mut s = CommitSearch { query: CommitQuery::parse("path:src"), ..Default::default() };
        s.paths.pending = Some(1);
        assert_eq!(s.count_text(None, whole(many)).as_deref(), Some("Searching files…"));
    }

    #[test]
    fn count_text_while_files_are_searched() {
        let mut s = CommitSearch { query: CommitQuery::parse("path:src"), ..Default::default() };
        s.paths.pending = Some(1);
        assert!(s.searching_files());
        assert_eq!(s.count_text(None, HistoryCount::Loaded).as_deref(), Some("Searching files…"));
        s.matches = vec![0, 3];
        assert_eq!(s.count_text(Some(3), HistoryCount::Loaded).as_deref(), Some("2 so far, searching…"));
        s.paths.pending = None;
        assert!(!s.searching_files());
        assert_eq!(s.count_text(Some(3), HistoryCount::Loaded).as_deref(), Some("2 of 2"));
        // A query without path terms isn't searching files, whatever is pending.
        let mut s = search(vec![1], false);
        s.paths.pending = Some(2);
        assert!(!s.searching_files());
        assert_eq!(s.count_text(None, HistoryCount::Loaded).as_deref(), Some("1 match"));
    }
}

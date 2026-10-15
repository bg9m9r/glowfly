//! The graph's commit search: the parsed query, its matches, the filter flag and the author
//! list, plus which rows the graph lays out while it filters (`GraphRows`).

use git2::Oid;
use std::collections::HashMap;
use trellis_core::commit_query::{AuthorCount, CommitQuery};
use trellis_core::repo::Snapshot;

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
    /// The loaded history's distinct authors for the author picker, worked out when it first
    /// opens and dropped whenever the history changes.
    pub authors: Option<Vec<AuthorCount>>,
}

impl CommitSearch {
    /// Whether there is something to search for.
    pub fn active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Where commit `commit_index` is among the matches (0-based), if it matches.
    pub fn position(&self, commit_index: usize) -> Option<usize> {
        self.matches.binary_search(&commit_index).ok()
    }

    /// The match count for the search bar ("No matches", "2 of 5", "5 matches", "1 match");
    /// `selected` is the selected commit's index. `None` when there is no search.
    pub fn count_text(&self, selected: Option<usize>) -> Option<String> {
        if !self.active() {
            return None;
        }
        let m = self.matches.len();
        Some(match (m, selected.and_then(|i| self.position(i))) {
            (0, _) => "No matches".to_owned(),
            (_, Some(n)) => format!("{} of {m}", n + 1),
            (1, None) => "1 match".to_owned(),
            (_, None) => format!("{m} matches"),
        })
    }
}

/// Which commits the graph lays out: all of them (after the WIP row, if any), or only the
/// search's matches while it filters.
pub struct GraphRows<'a> {
    /// How many commits the history has.
    commits: usize,
    row_of: &'a HashMap<Oid, usize>,
    /// Whether the history has a WIP row (shown unless filtered).
    wip: bool,
    shown: Option<&'a [usize]>,
}

impl<'a> GraphRows<'a> {
    /// The rows for `snap`: filtered when the bar is shown, the filter is on and there is a search.
    pub fn new(snap: &'a Snapshot, search: &'a CommitSearch, bar_shown: bool) -> Self {
        let filtered = bar_shown && search.filter && search.active();
        Self::from_parts(snap.commits.len(), &snap.row_of, snap.graph.wip.is_some(), filtered.then_some(search.matches.as_slice()))
    }

    fn from_parts(commits: usize, row_of: &'a HashMap<Oid, usize>, wip: bool, shown: Option<&'a [usize]>) -> Self {
        GraphRows { commits, row_of, wip, shown }
    }

    /// Whether only matching commits are shown.
    pub fn filtered(&self) -> bool {
        self.shown.is_some()
    }

    /// The commit indexes shown, when filtered.
    pub fn shown(&self) -> Option<&'a [usize]> {
        self.shown
    }

    /// Whether the first row is the working tree's (never while filtered).
    pub fn has_wip(&self) -> bool {
        self.wip && !self.filtered()
    }

    /// How many rows the graph has, counting the WIP row.
    pub fn len(&self) -> usize {
        match self.shown {
            Some(s) => s.len(),
            None => self.commits + self.has_wip() as usize,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The commit index shown in display row `row`; `None` for the WIP row, past the end, or an
    /// index the history doesn't have.
    pub fn commit_at(&self, row: usize) -> Option<usize> {
        let i = match self.shown {
            Some(s) => *s.get(row)?,
            None => row.checked_sub(self.has_wip() as usize)?,
        };
        (i < self.commits).then_some(i)
    }

    /// The display row of commit `oid`; `None` when it isn't loaded or the filter hides it.
    pub fn row_of(&self, oid: Oid) -> Option<usize> {
        let i = *self.row_of.get(&oid)?;
        match self.shown {
            Some(s) => s.binary_search(&i).ok(),
            None => Some(i + self.has_wip() as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(i: usize) -> Oid {
        Oid::from_str(&format!("{:040x}", i + 1)).unwrap()
    }

    /// `n` commits' rows.
    fn row_of(n: usize) -> HashMap<Oid, usize> {
        (0..n).map(|i| (oid(i), i)).collect()
    }

    fn search(matches: Vec<usize>, filter: bool) -> CommitSearch {
        CommitSearch { filter, query: CommitQuery::parse("commit"), matches, ..Default::default() }
    }

    /// The rows `GraphRows::new` would build for `n` commits.
    fn rows<'a>(n: usize, map: &'a HashMap<Oid, usize>, wip: bool, q: &'a CommitSearch, bar: bool) -> GraphRows<'a> {
        let filtered = bar && q.filter && q.active();
        GraphRows::from_parts(n, map, wip, filtered.then_some(q.matches.as_slice()))
    }

    #[test]
    fn unfiltered_rows_count_the_wip_row() {
        let map = row_of(4);
        let q = search(vec![1, 3], false);
        let r = rows(4, &map, true, &q, true);
        assert!(!r.filtered() && r.has_wip());
        assert_eq!(r.len(), 5);
        assert_eq!(r.commit_at(0), None);
        assert_eq!(r.commit_at(1), Some(0));
        assert_eq!(r.commit_at(4), Some(3));
        assert_eq!(r.commit_at(5), None);
        assert_eq!(r.row_of(oid(2)), Some(3));
        let r = rows(4, &map, false, &q, true);
        assert_eq!(r.len(), 4);
        assert_eq!(r.row_of(oid(2)), Some(2));
    }

    #[test]
    fn filtered_rows_are_the_matches() {
        let map = row_of(4);
        let q = search(vec![1, 3, 9], true);
        let r = rows(4, &map, true, &q, true);
        assert!(r.filtered() && !r.has_wip());
        assert_eq!(r.len(), 3);
        assert_eq!(r.shown(), Some(&[1, 3, 9][..]));
        assert_eq!(r.commit_at(0), Some(1));
        assert_eq!(r.commit_at(1), Some(3));
        assert_eq!(r.commit_at(2), None, "a stale index past the history");
        assert_eq!(r.row_of(oid(3)), Some(1));
        assert_eq!(r.row_of(oid(2)), None, "hidden by the filter");
        assert_eq!(r.row_of(oid(99)), None);
    }

    #[test]
    fn rows_filter_only_with_the_bar_shown_and_a_query() {
        let map = row_of(3);
        let q = search(vec![1], true);
        assert!(!rows(3, &map, false, &q, false).filtered());
        let empty = CommitSearch { filter: true, ..Default::default() };
        assert!(!rows(3, &map, false, &empty, true).filtered());
        let none = search(vec![], true);
        let r = rows(3, &map, true, &none, true);
        assert!(r.filtered() && r.is_empty() && !r.has_wip());
    }

    #[test]
    fn count_text_describes_the_matches() {
        assert_eq!(CommitSearch::default().count_text(Some(0)), None);
        assert_eq!(search(vec![], false).count_text(None).as_deref(), Some("No matches"));
        assert_eq!(search(vec![2], false).count_text(None).as_deref(), Some("1 match"));
        assert_eq!(search(vec![2], false).count_text(Some(2)).as_deref(), Some("1 of 1"));
        assert_eq!(search(vec![0, 2, 5], false).count_text(Some(1)).as_deref(), Some("3 matches"));
        assert_eq!(search(vec![0, 2, 5], false).count_text(Some(2)).as_deref(), Some("2 of 3"));
    }
}

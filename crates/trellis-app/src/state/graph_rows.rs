//! Which rows the commit graph lays out (`GraphRows`): this tab's WIP row, other worktrees'
//! WIP rows above their HEAD commits, then the commits; or only the search's matching commits
//! while it filters.

use crate::state::search::CommitSearch;
use git2::Oid;
use std::cmp::Ordering;
use std::collections::HashMap;
use trellis_core::graph::WorktreeWipRow;
use trellis_core::repo::Snapshot;

/// What one display row of the graph shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayRow {
    /// This tab's uncommitted changes.
    Wip,
    /// Another worktree's uncommitted changes: an index into `snap.graph.worktree_wips`.
    Worktree(usize),
    /// A commit: an index into `snap.commits`.
    Commit(usize),
}

/// Which rows the graph lays out: this tab's WIP row (if any), then the commits with each other
/// worktree's WIP row directly above its HEAD commit; or only the search's matches while it
/// filters.
pub struct GraphRows<'a> {
    /// How many commits the history has.
    commits: usize,
    row_of: &'a HashMap<Oid, usize>,
    /// Whether the history has a WIP row (shown unless filtered).
    wip: bool,
    /// Other worktrees' WIP rows, sorted by the commit they sit above (shown unless filtered).
    extra: &'a [WorktreeWipRow],
    shown: Option<&'a [usize]>,
}

impl<'a> GraphRows<'a> {
    /// The rows for `snap`: filtered when the bar is shown, the filter is on and there is a search.
    pub fn new(snap: &'a Snapshot, search: &'a CommitSearch, bar_shown: bool) -> Self {
        let filtered = bar_shown && search.filter && search.active();
        let (wip, extra) = (snap.graph.wip.is_some(), snap.graph.worktree_wips.as_slice());
        Self::from_parts(snap.commits.len(), &snap.row_of, wip, extra, filtered.then_some(search.matches.as_slice()))
    }

    fn from_parts(
        commits: usize,
        row_of: &'a HashMap<Oid, usize>,
        wip: bool,
        extra: &'a [WorktreeWipRow],
        shown: Option<&'a [usize]>,
    ) -> Self {
        GraphRows { commits, row_of, wip, extra, shown }
    }

    /// The other worktrees' WIP rows shown (none while filtered).
    fn extra(&self) -> &'a [WorktreeWipRow] {
        if self.filtered() { &[] } else { self.extra }
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

    /// How many rows the graph has, counting the WIP rows.
    pub fn len(&self) -> usize {
        match self.shown {
            Some(s) => s.len(),
            None => self.commits + self.has_wip() as usize + self.extra().len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// What display row `row` shows; `None` past the end or for an index the history doesn't have.
    pub fn at(&self, row: usize) -> Option<DisplayRow> {
        if let Some(s) = self.shown {
            let i = *s.get(row)?;
            return (i < self.commits).then_some(DisplayRow::Commit(i));
        }
        if self.has_wip() && row == 0 {
            return Some(DisplayRow::Wip);
        }
        let r = row - self.has_wip() as usize;
        // Worktree row k sits at r == before + k: the k worktree rows above it push it down.
        let mut above = 0;
        for (k, w) in self.extra.iter().enumerate() {
            match (w.before + k).cmp(&r) {
                Ordering::Equal => return Some(DisplayRow::Worktree(k)),
                Ordering::Less => above += 1,
                Ordering::Greater => break,
            }
        }
        let i = r - above;
        (i < self.commits).then_some(DisplayRow::Commit(i))
    }

    /// The display row of commit `oid`; `None` when it isn't loaded or the filter hides it.
    pub fn row_of(&self, oid: Oid) -> Option<usize> {
        let i = *self.row_of.get(&oid)?;
        match self.shown {
            Some(s) => s.binary_search(&i).ok(),
            None => Some(i + self.has_wip() as usize + self.extra.partition_point(|w| w.before <= i)),
        }
    }

    /// The row Up (`step` -1) or Down (+1) selects from `current`: the nearest row that way that
    /// can be selected (other worktrees' rows can't), staying put at either end. With nothing
    /// selected it is the first selectable row. `None` when only worktree rows lie that way.
    pub fn step(&self, current: Option<usize>, step: i64) -> Option<usize> {
        let len = self.len() as i64;
        if len == 0 {
            return None;
        }
        let dir = if current.is_some() && step < 0 { -1 } else { 1 };
        let mut next = current.map_or(0, |c| (c as i64 + step).clamp(0, len - 1));
        while matches!(self.at(next as usize), Some(DisplayRow::Worktree(_))) {
            next += dir;
            if !(0..len).contains(&next) {
                return None;
            }
        }
        Some(next as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::commit_query::CommitQuery;

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
        with_extra(n, map, wip, &[], q, bar)
    }

    fn with_extra<'a>(
        n: usize,
        map: &'a HashMap<Oid, usize>,
        wip: bool,
        extra: &'a [WorktreeWipRow],
        q: &'a CommitSearch,
        bar: bool,
    ) -> GraphRows<'a> {
        let filtered = bar && q.filter && q.active();
        GraphRows::from_parts(n, map, wip, extra, filtered.then_some(q.matches.as_slice()))
    }

    /// Worktree WIP rows above the commits at `before`.
    fn extra(before: &[usize]) -> Vec<WorktreeWipRow> {
        before.iter().enumerate().map(|(k, &b)| WorktreeWipRow { worktree: k, before: b, row: Default::default() }).collect()
    }

    #[test]
    fn worktree_rows_sit_above_their_commits() {
        use DisplayRow::*;
        let (map, ex) = (row_of(4), extra(&[0, 2, 2]));
        let q = search(vec![1, 3], false);
        let r = with_extra(4, &map, true, &ex, &q, true);
        assert_eq!(r.len(), 8);
        let all: Vec<Option<DisplayRow>> = (0..9).map(|i| r.at(i)).collect();
        let want = [Wip, Worktree(0), Commit(0), Commit(1), Worktree(1), Worktree(2), Commit(2), Commit(3)];
        assert_eq!(all, want.iter().copied().map(Some).chain([None]).collect::<Vec<_>>());
        assert_eq!((r.row_of(oid(2)), r.row_of(oid(0)), r.row_of(oid(3))), (Some(6), Some(2), Some(7)));

        let r = with_extra(4, &map, false, &ex, &q, true);
        assert_eq!(r.len(), 7);
        assert_eq!((r.at(0), r.at(1), r.at(5)), (Some(Worktree(0)), Some(Commit(0)), Some(Commit(2))));
        assert_eq!((r.row_of(oid(2)), r.row_of(oid(0))), (Some(5), Some(1)));

        let q = search(vec![1, 3], true);
        let r = with_extra(4, &map, true, &ex, &q, true);
        assert_eq!(r.len(), 2, "no WIP rows while filtered");
        assert_eq!((r.at(0), r.at(1), r.at(2)), (Some(Commit(1)), Some(Commit(3)), None));
        assert_eq!((r.row_of(oid(3)), r.row_of(oid(2))), (Some(1), None));
    }

    #[test]
    fn arrow_steps_skip_worktree_rows() {
        let (map, ex) = (row_of(4), extra(&[0, 2, 2]));
        let q = search(vec![], false);
        let r = with_extra(4, &map, true, &ex, &q, true);
        assert_eq!(r.step(Some(2), -1), Some(0), "up from the first commit to the own WIP row");
        assert_eq!(r.step(Some(3), 1), Some(6));
        assert_eq!(r.step(Some(6), -1), Some(3));
        assert_eq!(r.step(Some(7), 1), Some(7), "the last row stays");
        assert_eq!(r.step(None, 1), Some(0));

        let ex = extra(&[0]);
        let r = with_extra(4, &map, false, &ex, &q, true);
        assert_eq!(r.step(Some(1), -1), None, "only a worktree row above");
        assert_eq!(r.step(None, 1), Some(1), "nothing selected: the first selectable row");
        assert_eq!(r.step(Some(1), 1), Some(2));
    }

    #[test]
    fn unfiltered_rows_count_the_wip_row() {
        let map = row_of(4);
        let q = search(vec![1, 3], false);
        let r = rows(4, &map, true, &q, true);
        assert!(!r.filtered() && r.has_wip());
        assert_eq!(r.len(), 5);
        assert_eq!(r.at(0), Some(DisplayRow::Wip));
        assert_eq!(r.at(1), Some(DisplayRow::Commit(0)));
        assert_eq!(r.at(4), Some(DisplayRow::Commit(3)));
        assert_eq!(r.at(5), None);
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
        assert_eq!(r.at(0), Some(DisplayRow::Commit(1)));
        assert_eq!(r.at(1), Some(DisplayRow::Commit(3)));
        assert_eq!(r.at(2), None, "a stale index past the history");
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
}

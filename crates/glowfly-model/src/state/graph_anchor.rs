//! Keeping the graph's top visible commit in place across a reload: the commits from the top
//! visible row down, with their rows, taken before a new snapshot replaces the old, and the
//! scroll offset that puts the first one still shown at the same screen position.

use crate::state::graph_rows::{DisplayRow, GraphRows};
use git2::Oid;
use glowfly_core::repo::Snapshot;

/// How far below the top row the fallback looks for a commit still shown.
const SCAN_ROWS: usize = 256;
/// An offset under this is the top of the graph: new commits show above, nothing is anchored.
const AT_TOP: f32 = 1.0;

/// The graph's pending scroll requests (`ViewState::scroll_to_row`, `scroll_offset`).
pub type Pending = (Option<usize>, Option<f32>);

/// The commits on screen before a reload, to scroll the first one still shown back to its place.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphAnchor {
    /// The scroll offset the rows were read at.
    offset: f32,
    /// The graph's row height then.
    row_h: f32,
    /// The commit rows from the top visible row down: (display row then, commit).
    rows: Vec<(usize, Oid)>,
    /// The scroll requests pending when it was taken.
    pending: Pending,
}

impl GraphAnchor {
    /// The anchor for `rows` of `snap` scrolled to `offset`; None at the top, before the graph
    /// was drawn, or with no commit row in reach.
    pub fn take(rows: &GraphRows, snap: &Snapshot, offset: f32, row_h: f32, pending: Pending) -> Option<Self> {
        if row_h.is_nan() || row_h <= 0.0 || offset < AT_TOP {
            return None;
        }
        let top = (offset / row_h) as usize;
        let from: Vec<(usize, Oid)> = (top..rows.len().min(top + SCAN_ROWS))
            .filter_map(|r| match rows.at(r) {
                Some(DisplayRow::Commit(i)) => snap.commits.get(i).map(|c| (r, c.oid)),
                _ => None,
            })
            .collect();
        (!from.is_empty()).then_some(GraphAnchor { offset, row_h, rows: from, pending })
    }

    /// Whether nothing asked to scroll since it was taken.
    pub fn still_wanted(&self, now: Pending) -> bool {
        self.pending == now
    }

    /// The offset that keeps the first of its commits still in `rows` where it was; None when
    /// none is.
    pub fn offset_in(&self, rows: &GraphRows) -> Option<f32> {
        shifted(self.offset, self.row_h, &self.rows, |o| rows.row_of(o))
    }

    /// The offset it was taken at.
    pub fn offset(&self) -> f32 {
        self.offset
    }
}

/// `offset` moved by the first of `from` that `row_of` still finds, by its change of row.
fn shifted(offset: f32, row_h: f32, from: &[(usize, Oid)], row_of: impl Fn(Oid) -> Option<usize>) -> Option<f32> {
    from.iter().find_map(|&(old, oid)| row_of(oid).map(|new| (offset + (new as f32 - old as f32) * row_h).max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn oid(i: usize) -> Oid {
        Oid::from_str(&format!("{:040x}", i + 1)).unwrap()
    }

    fn rows_now(at: &[(usize, usize)]) -> HashMap<Oid, usize> {
        at.iter().map(|&(i, row)| (oid(i), row)).collect()
    }

    #[test]
    fn moves_by_whole_rows_keeping_the_part_scrolled_off() {
        let now = rows_now(&[(0, 103)]);
        let got = shifted(3610.0, 36.0, &[(100, oid(0)), (101, oid(1))], |o| now.get(&o).copied());
        assert_eq!(got, Some(3718.0));
    }

    #[test]
    fn falls_back_to_the_next_commit_still_shown() {
        let now = rows_now(&[(1, 98), (2, 99)]);
        let got = shifted(3610.0, 36.0, &[(100, oid(0)), (101, oid(1)), (102, oid(2))], |o| now.get(&o).copied());
        assert_eq!(got, Some(3610.0 - 3.0 * 36.0), "anchored on the second: row 101 -> 98");
    }

    #[test]
    fn none_when_no_commit_is_still_shown() {
        let now = rows_now(&[(7, 3)]);
        assert_eq!(shifted(3610.0, 36.0, &[(100, oid(0)), (101, oid(1))], |o| now.get(&o).copied()), None);
        assert_eq!(shifted(3610.0, 36.0, &[], |o| now.get(&o).copied()), None);
    }

    #[test]
    fn a_row_moved_up_past_the_top_clamps_to_zero() {
        let now = rows_now(&[(0, 0)]);
        // A fallback commit five rows down from the top row 2 is now the first row.
        assert_eq!(shifted(100.0, 36.0, &[(7, oid(0))], |o| now.get(&o).copied()), Some(0.0), "100 - 7 * 36 < 0");
    }
}

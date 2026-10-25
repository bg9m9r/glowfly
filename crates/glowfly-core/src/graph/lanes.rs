//! Laying commits out in lanes, row by row (the rules are in the `graph` module docs). The lanes
//! below the last row are kept (`LaneTail`), so more commits of the same history can be laid out
//! under the rows already made (`Layout::extend`) with the rows a layout of all of them gives.

use super::{Edges, GraphRow, LaneSet, Layout, WorktreeWipRow};
use crate::repo::CommitInfo;
use git2::Oid;
use smallvec::SmallVec;
use std::collections::HashMap;

/// Where the lanes stand below a layout's last row: the commit each lane waits for, and the other
/// worktrees (by index) whose HEAD commit hasn't been laid out yet.
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct LaneTail {
    lanes: Vec<Option<Oid>>,
    waiting: HashMap<Oid, Vec<usize>>,
}

fn free_slot(lanes: &mut Vec<Option<Oid>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// Lays out `commits` (topological order), with this worktree's WIP row above `head` when `wip`,
/// and a WIP row for each (worktree index, HEAD commit) in `worktrees` whose commit is loaded.
pub fn layout(commits: &[CommitInfo], head: Option<Oid>, wip: bool, worktrees: &[(usize, Oid)]) -> Layout {
    let mut out = start(head, wip, worktrees);
    out.extend(commits, 0);
    out
}

/// A layout with no commit rows yet: this worktree's WIP row above `head` when `wip`, and the
/// other worktrees waiting for their HEAD commits.
pub(crate) fn start(head: Option<Oid>, wip: bool, worktrees: &[(usize, Oid)]) -> Layout {
    let mut out = Layout::default();
    for &(k, oid) in worktrees {
        out.tail.waiting.entry(oid).or_default().push(k);
    }
    if wip {
        let mut row = GraphRow::default();
        if let Some(h) = head {
            // Reserve lane 0 for HEAD so the WIP node connects straight down to it.
            out.tail.lanes.push(Some(h));
            row.bottom.push((0, 0));
        }
        out.wip = Some(row);
        out.max_width = 1;
    }
    out
}

impl Layout {
    /// Lays `commits` out below the last row, as the commits from index `first` of the history
    /// (`rows.len()` must be `first`).
    pub fn extend(&mut self, commits: &[CommitInfo], first: usize) {
        let LaneTail { lanes, waiting } = &mut self.tail;
        for (i, c) in commits.iter().enumerate() {
            let idx = first + i;
            // Other worktrees at this commit: a node in a free lane whose only parent is `c`.
            if !waiting.is_empty()
                && let Some(here) = waiting.remove(&c.oid)
            {
                for worktree in here {
                    let col = free_slot(lanes);
                    let through = (0..lanes.len()).filter(|&i| lanes[i].is_some()).collect();
                    lanes[col] = Some(c.oid);
                    let row = GraphRow { col, top: Edges::new(), bottom: Edges::from_slice(&[(col, col)]), through };
                    self.worktree_wips.push(WorktreeWipRow { worktree, before: idx, row });
                    self.max_width = self.max_width.max(lanes.len());
                }
            }
            let matches: SmallVec<[usize; 4]> = (0..lanes.len()).filter(|&i| lanes[i] == Some(c.oid)).collect();
            let col = match matches.first() {
                Some(&i) => i,
                None => free_slot(lanes),
            };
            let through: LaneSet = (0..lanes.len()).filter(|&i| lanes[i].is_some() && lanes[i] != Some(c.oid)).collect();
            let top = matches.iter().map(|&i| (i, col)).collect();
            for &i in &matches {
                lanes[i] = None;
            }

            let mut bottom = Edges::new();
            if let Some(&first) = c.parents.first() {
                lanes[col] = Some(first);
                bottom.push((col, col));
            }
            for p in c.parents.iter().skip(1) {
                let lane = match lanes.iter().position(|l| *l == Some(*p)) {
                    Some(j) => j,
                    None => {
                        let k = free_slot(lanes);
                        lanes[k] = Some(*p);
                        k
                    }
                };
                bottom.push((col, lane));
            }

            self.max_width = self.max_width.max(lanes.len()).max(col + 1);
            while lanes.last() == Some(&None) {
                lanes.pop();
            }
            self.rows.push(GraphRow { col, top, bottom, through });
        }
    }
}

#[cfg(test)]
#[path = "lanes_tests.rs"]
mod tests;

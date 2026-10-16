//! Lane assignment for the commit graph.
//!
//! Commits arrive in topological order (children before parents). Each lane holds the commit it
//! is waiting for; a commit takes the first lane waiting for it, and its parents then occupy lanes
//! below it. Lanes never shift sideways, so pass-through lines stay vertical.
//!
//! Other worktrees with uncommitted changes get a WIP row directly above their HEAD commit: each
//! is laid out as a commit whose only parent is that HEAD and which nothing waits for.

use crate::repo::CommitInfo;
use git2::Oid;
use std::collections::HashMap;

#[derive(Default, Clone, Debug, PartialEq)]
pub struct GraphRow {
    /// Lane holding this row's node.
    pub col: usize,
    /// Lines from a lane at the top of the row into the node: (lane, col).
    pub top: Vec<(usize, usize)>,
    /// Lines from the node to a lane at the bottom of the row: (col, lane).
    pub bottom: Vec<(usize, usize)>,
    /// Lanes passing straight through the row.
    pub through: LaneSet,
}

/// A set of lane numbers, kept in the row itself up to 64 lanes: a graph of the 200k-commit
/// limit with many lanes would otherwise spend hundreds of MB on per-row lists.
#[derive(Clone, Debug, PartialEq)]
pub enum LaneSet {
    Small(u64),
    Large(Box<[u64]>),
}

impl Default for LaneSet {
    fn default() -> Self {
        LaneSet::Small(0)
    }
}

impl FromIterator<usize> for LaneSet {
    fn from_iter<I: IntoIterator<Item = usize>>(lanes: I) -> Self {
        let mut words = vec![0u64];
        for i in lanes {
            let w = i / 64;
            if w >= words.len() {
                words.resize(w + 1, 0);
            }
            words[w] |= 1 << (i % 64);
        }
        match words[..] {
            [one] => LaneSet::Small(one),
            _ => LaneSet::Large(words.into_boxed_slice()),
        }
    }
}

impl LaneSet {
    fn words(&self) -> &[u64] {
        match self {
            LaneSet::Small(w) => std::slice::from_ref(w),
            LaneSet::Large(ws) => ws,
        }
    }

    /// The lanes in increasing order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words().iter().enumerate().flat_map(|(n, &w)| (0..64).filter(move |b| w & (1 << b) != 0).map(move |b| n * 64 + b))
    }

    pub fn contains(&self, lane: usize) -> bool {
        self.words().get(lane / 64).is_some_and(|w| w & (1 << (lane % 64)) != 0)
    }

    pub fn len(&self) -> usize {
        self.words().iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The WIP row of another worktree, drawn directly above the row of its HEAD commit.
#[derive(Clone, Debug, PartialEq)]
pub struct WorktreeWipRow {
    /// Index into `Snapshot::worktrees` (as passed to `layout`).
    pub worktree: usize,
    /// Index into the commits of its HEAD commit: the row it is drawn directly above.
    pub before: usize,
    /// Its lanes.
    pub row: GraphRow,
}

#[derive(Default)]
pub struct Layout {
    /// Row for uncommitted changes, drawn above HEAD.
    pub wip: Option<GraphRow>,
    /// `rows[i]` is the row of `commits[i]`.
    pub rows: Vec<GraphRow>,
    /// Other worktrees' WIP rows, sorted by `before`; worktrees at the same commit in the order
    /// they were passed in.
    pub worktree_wips: Vec<WorktreeWipRow>,
    pub max_width: usize,
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
    let mut lanes: Vec<Option<Oid>> = Vec::new();
    let mut out = Layout::default();
    let mut waiting: HashMap<Oid, Vec<usize>> = HashMap::new();
    for &(k, oid) in worktrees {
        waiting.entry(oid).or_default().push(k);
    }

    if wip {
        let mut row = GraphRow::default();
        if let Some(h) = head {
            // Reserve lane 0 for HEAD so the WIP node connects straight down to it.
            lanes.push(Some(h));
            row.bottom.push((0, 0));
        }
        out.wip = Some(row);
        out.max_width = 1;
    }

    for (idx, c) in commits.iter().enumerate() {
        // Other worktrees at this commit: a node in a free lane whose only parent is `c`.
        if !waiting.is_empty()
            && let Some(here) = waiting.remove(&c.oid)
        {
            for worktree in here {
                let col = free_slot(&mut lanes);
                let through = (0..lanes.len()).filter(|&i| lanes[i].is_some()).collect();
                lanes[col] = Some(c.oid);
                let row = GraphRow { col, top: vec![], bottom: vec![(col, col)], through };
                out.worktree_wips.push(WorktreeWipRow { worktree, before: idx, row });
                out.max_width = out.max_width.max(lanes.len());
            }
        }
        let matches: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i] == Some(c.oid)).collect();
        let col = match matches.first() {
            Some(&i) => i,
            None => free_slot(&mut lanes),
        };
        let through: LaneSet = (0..lanes.len()).filter(|&i| lanes[i].is_some() && lanes[i] != Some(c.oid)).collect();
        let top = matches.iter().map(|&i| (i, col)).collect();
        for &i in &matches {
            lanes[i] = None;
        }

        let mut bottom = Vec::new();
        if let Some(&first) = c.parents.first() {
            lanes[col] = Some(first);
            bottom.push((col, col));
        }
        for p in c.parents.iter().skip(1) {
            let lane = match lanes.iter().position(|l| *l == Some(*p)) {
                Some(j) => j,
                None => {
                    let k = free_slot(&mut lanes);
                    lanes[k] = Some(*p);
                    k
                }
            };
            bottom.push((col, lane));
        }

        out.max_width = out.max_width.max(lanes.len()).max(col + 1);
        while lanes.last() == Some(&None) {
            lanes.pop();
        }
        out.rows.push(GraphRow { col, top, bottom, through });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    /// A -> B -> C (root), newest first.
    fn chain() -> Vec<CommitInfo> {
        let c = |n: u8, parents: Vec<Oid>| CommitInfo {
            oid: oid(n),
            summary: String::new(),
            body: String::new(),
            author: String::new(),
            email: String::new(),
            time: 0,
            parents,
        };
        vec![c(1, vec![oid(2)]), c(2, vec![oid(3)]), c(3, vec![])]
    }

    fn lanes(r: &GraphRow) -> Vec<usize> {
        r.through.iter().collect()
    }

    #[test]
    fn a_worktree_row_takes_a_free_lane_and_joins_its_commit() {
        let commits = chain();
        let l = layout(&commits, Some(oid(1)), false, &[(0, oid(2))]);
        let [w] = &l.worktree_wips[..] else { panic!("{:?}", l.worktree_wips) };
        assert_eq!((w.worktree, w.before), (0, 1));
        assert_eq!((w.row.col, w.row.top.clone(), w.row.bottom.clone(), lanes(&w.row)), (1, vec![], vec![(1, 1)], vec![0]));
        assert!(l.rows[1].top.contains(&(0, 0)) && l.rows[1].top.contains(&(1, 0)), "{:?}", l.rows[1]);
        assert_eq!((l.rows.len(), l.max_width), (3, 2));
        assert!(l.rows[2].through.is_empty());
    }

    #[test]
    fn a_worktree_at_head_sits_below_the_own_wip_row() {
        let commits = chain();
        let l = layout(&commits, Some(oid(1)), true, &[(3, oid(1))]);
        assert_eq!(l.wip.as_ref().unwrap().bottom, [(0, 0)]);
        let [w] = &l.worktree_wips[..] else { panic!() };
        assert_eq!((w.worktree, w.before, w.row.col), (3, 0, 1));
        assert!(w.row.through.contains(0));
        assert_eq!(l.rows[0].top, [(0, 0), (1, 0)]);
    }

    #[test]
    fn worktrees_at_one_commit_get_a_row_each_in_order() {
        let commits = chain();
        let l = layout(&commits, Some(oid(1)), false, &[(1, oid(3)), (2, oid(3))]);
        let got: Vec<(usize, usize, usize)> = l.worktree_wips.iter().map(|w| (w.worktree, w.before, w.row.col)).collect();
        assert_eq!(got, [(1, 2, 1), (2, 2, 2)]);
        assert_eq!(l.rows[2].top, [(0, 0), (1, 0), (2, 0)]);
        assert_eq!(l.max_width, 3);
    }

    #[test]
    fn without_worktree_rows_the_layout_is_unchanged() {
        let commits = chain();
        let plain = layout(&commits, Some(oid(1)), true, &[]);
        let unknown = layout(&commits, Some(oid(1)), true, &[(1, oid(9))]);
        assert!(plain.worktree_wips.is_empty() && unknown.worktree_wips.is_empty(), "a head outside the history gets no row");
        let straight =
            |top: Vec<(usize, usize)>, bottom: Vec<(usize, usize)>| GraphRow { col: 0, top, bottom, through: LaneSet::default() };
        let want = vec![straight(vec![(0, 0)], vec![(0, 0)]), straight(vec![(0, 0)], vec![(0, 0)]), straight(vec![(0, 0)], vec![])];
        assert_eq!(plain.rows, want);
        assert_eq!(unknown.rows, want);
        assert_eq!((plain.max_width, unknown.max_width), (1, 1));
    }

    #[test]
    fn lane_sets_hold_any_lanes_in_order() {
        let small: LaneSet = [0, 3, 63].into_iter().collect();
        assert!(matches!(small, LaneSet::Small(_)), "no allocation up to 64 lanes");
        assert_eq!(small.iter().collect::<Vec<_>>(), [0, 3, 63]);
        let large: LaneSet = [1, 64, 200].into_iter().collect();
        assert_eq!(large.iter().collect::<Vec<_>>(), [1, 64, 200]);
        assert!(large.contains(64) && !large.contains(65) && !large.contains(9999));
        assert_eq!((large.len(), LaneSet::default().is_empty()), (3, true));
    }
}

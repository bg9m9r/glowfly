//! Lane assignment for the commit graph.
//!
//! Commits arrive in topological order (children before parents). Each lane holds the commit it
//! is waiting for; a commit takes the first lane waiting for it, and its parents then occupy lanes
//! below it. Lanes never shift sideways, so pass-through lines stay vertical.

use crate::repo::CommitInfo;
use git2::Oid;

#[derive(Default, Clone)]
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

#[derive(Default)]
pub struct Layout {
    /// Row for uncommitted changes, drawn above HEAD.
    pub wip: Option<GraphRow>,
    pub rows: Vec<GraphRow>,
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

pub fn layout(commits: &[CommitInfo], head: Option<Oid>, wip: bool) -> Layout {
    let mut lanes: Vec<Option<Oid>> = Vec::new();
    let mut out = Layout::default();

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

    for c in commits {
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
    use super::LaneSet;

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

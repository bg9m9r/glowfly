//! Which branch owns each part of the graph: every commit row's node, the WIP rows and every lane
//! from row to row, with the pairs of owners that ever sit in side-by-side lanes.
//!
//! A branch owns its first-parent line (`git log --first-parent`) from its tip down. The main
//! branch takes over from its own tip and wherever its line joins another, so its history below
//! a fork stays its own. A label on a commit doesn't recolour a line another label already owns.
//! A merge's other parents start lanes named after the branches its message merged
//! (`merge_names`), else unnamed lanes known by the merged commit.

use super::Layout;
use super::merge_names::merged_names;
use crate::repo::CommitInfo;
use git2::Oid;
use std::collections::HashMap;

/// Who a part of the graph belongs to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Owner {
    /// A branch, by its colour key (`branch_colour::branch_key`).
    Branch(String),
    /// A line no branch names, known by the commit that started it.
    Unnamed(Oid),
}

/// A row's place in display order: this tab's WIP row, then each commit after the other
/// worktrees' WIP rows drawn above it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct RowPos(pub u32);

impl RowPos {
    pub fn wip() -> Self {
        RowPos(0)
    }

    /// Another worktree's WIP row, drawn above commit `before`.
    pub fn worktree(before: usize) -> Self {
        RowPos(3 * before as u32 + 1)
    }

    pub fn commit(i: usize) -> Self {
        RowPos(3 * i as u32 + 2)
    }
}

/// What names the lanes: the branch keys at each commit and the checked-out branches.
#[derive(Default)]
pub struct LaneNames<'a> {
    /// Branch keys at each commit, best first (HEAD's branch, the other local branches in label
    /// order, then keys only on remotes), without repeats.
    pub tips: HashMap<Oid, Vec<String>>,
    pub head_branch: Option<&'a str>,
    /// HEAD's commit: what names this tab's WIP row when HEAD is detached.
    pub head: Option<Oid>,
    /// The repository's main branch (`Snapshot::mainline`).
    pub mainline: Option<String>,
    pub remotes: Vec<&'a str>,
    /// Each worktree's checked-out branch, indexed like `Snapshot::worktrees`.
    pub worktree_branches: Vec<Option<String>>,
}

/// The owners of a laid-out graph. Owners are numbered in order of first appearance from the top.
#[derive(Default, Debug)]
pub struct Owners {
    pub list: Vec<Owner>,
    pub mainline: Option<String>,
    /// The owner of each commit row's node.
    pub commit: Vec<u32>,
    /// The owner of this tab's WIP row.
    pub wip: Option<u32>,
    /// The owner of each other worktree's WIP row, parallel to `Layout::worktree_wips`.
    pub worktree_wips: Vec<u32>,
    /// Per lane, (row, owner) from that row down, rows ascending; `ENDED` where a line stops.
    pub(crate) runs: Vec<Vec<(u32, u32)>>,
    /// Pairs of owners that sit in side-by-side lanes somewhere, (lower, higher), sorted.
    pub neighbours: Vec<(u32, u32)>,
    pub(crate) by_key: HashMap<String, u32>,
}

/// A run's owner where the lane's line stops.
const ENDED: u32 = u32::MAX;

impl Owners {
    /// The owner of `lane` coming into the row at `p` from above.
    pub fn lane_before(&self, lane: usize, p: RowPos) -> Option<u32> {
        let runs = self.runs.get(lane)?;
        let k = runs.partition_point(|r| r.0 < p.0);
        k.checked_sub(1).map(|k| runs[k].1).filter(|&o| o != ENDED)
    }

    /// The owner of `lane` leaving the row at `p` downward.
    pub fn lane_after(&self, lane: usize, p: RowPos) -> Option<u32> {
        let runs = self.runs.get(lane)?;
        let k = runs.partition_point(|r| r.0 <= p.0);
        k.checked_sub(1).map(|k| runs[k].1).filter(|&o| o != ENDED)
    }

    /// The branch key of `owner`; None for an unnamed line.
    pub fn key(&self, owner: u32) -> Option<&str> {
        match self.list.get(owner as usize)? {
            Owner::Branch(k) => Some(k),
            Owner::Unnamed(_) => None,
        }
    }

    pub fn owner_of_key(&self, key: &str) -> Option<u32> {
        self.by_key.get(key).copied()
    }

    fn branch(&mut self, key: &str) -> u32 {
        if let Some(&i) = self.by_key.get(key) {
            return i;
        }
        let i = self.list.len() as u32;
        self.list.push(Owner::Branch(key.to_owned()));
        self.by_key.insert(key.to_owned(), i);
        i
    }

    fn unnamed(&mut self, oid: Oid) -> u32 {
        self.list.push(Owner::Unnamed(oid));
        self.list.len() as u32 - 1
    }
}

/// The replay's lanes: each lane's owner and whether a branch label (not a merge message) named it.
struct Replay {
    out: Owners,
    lanes: Vec<Option<(u32, bool)>>,
    pairs: Vec<(u32, u32)>,
}

impl Replay {
    fn set(&mut self, lane: usize, owner: u32, by_label: bool, p: RowPos) {
        if lane >= self.lanes.len() {
            self.lanes.resize(lane + 1, None);
            self.out.runs.resize_with(lane + 1, Vec::new);
        }
        self.lanes[lane] = Some((owner, by_label));
        let runs = &mut self.out.runs[lane];
        if runs.last().is_none_or(|r| r.1 != owner) {
            runs.push((p.0, owner));
        }
    }

    fn owner(&self, lane: usize) -> Option<(u32, bool)> {
        self.lanes.get(lane).copied().flatten()
    }

    /// Records `owner` (in `lane`) as a neighbour of whoever holds the lanes either side.
    fn meet(&mut self, lane: usize, owner: u32) {
        for j in [lane.checked_sub(1), Some(lane + 1)].into_iter().flatten() {
            if let Some((other, _)) = self.owner(j)
                && other != owner
            {
                self.pairs.push((owner.min(other), owner.max(other)));
            }
        }
    }
}

/// Works out the owners of `layout` (laid out from `commits`) with the lane names `names`.
pub fn assign(layout: &Layout, commits: &[CommitInfo], names: &LaneNames) -> Owners {
    let mut r = Replay { out: Owners { mainline: names.mainline.clone(), ..Owners::default() }, lanes: Vec::new(), pairs: Vec::new() };
    let main = names.mainline.as_deref();
    if let Some(w) = &layout.wip {
        let owner = match (names.head_branch, names.head) {
            (Some(b), _) => Some((r.out.branch(b), true)),
            (None, Some(h)) => Some((r.out.unnamed(h), false)),
            (None, None) => None,
        };
        r.out.wip = owner.map(|o| o.0);
        if let Some((o, by_label)) = owner
            && w.bottom.contains(&(0, 0))
        {
            r.set(0, o, by_label, RowPos::wip());
            r.meet(0, o);
        }
    }
    let mut wts = layout.worktree_wips.iter().peekable();
    for (i, (g, c)) in layout.rows.iter().zip(commits).enumerate() {
        while let Some(w) = wts.next_if(|w| w.before == i) {
            let branch = names.worktree_branches.get(w.worktree).cloned().flatten();
            let (o, by_label) = match branch {
                Some(b) => (r.out.branch(&b), true),
                None => (r.out.unnamed(c.oid), false),
            };
            r.out.worktree_wips.push(o);
            r.set(w.row.col, o, by_label, RowPos::worktree(i));
            r.meet(w.row.col, o);
        }
        let p = RowPos::commit(i);
        let tips = names.tips.get(&c.oid).map_or(&[][..], Vec::as_slice);
        let main_top = g.top.iter().find_map(|&(l, _)| r.owner(l).filter(|(o, _)| main.is_some() && r.out.key(*o) == main));
        let col_lane = r.owner(g.col);
        let (node, by_label) = if let Some(m) = main.filter(|m| tips.iter().any(|t| t == m)) {
            (r.out.branch(m), true)
        } else if let Some((o, _)) = main_top {
            (o, true)
        } else if let Some((o, true)) = col_lane {
            (o, true)
        } else if let Some(first) = tips.first() {
            (r.out.branch(first), true)
        } else if let (false, Some(lane)) = (g.top.is_empty(), col_lane) {
            lane
        } else {
            (r.out.unnamed(c.oid), false)
        };
        r.out.commit.push(node);
        for &(l, _) in &g.top {
            if let Some(slot) = r.lanes.get_mut(l) {
                *slot = None;
            }
        }
        let merged = if c.parents.len() > 1 { merged_names(&c.summary, &names.remotes) } else { Vec::new() };
        let mut k = 0;
        for &(_, j) in &g.bottom {
            if j == g.col {
                r.set(j, node, by_label, p);
            } else {
                k += 1;
                if r.owner(j).is_some() {
                    continue;
                }
                let (o, by) = match merged.get(k - 1) {
                    Some(name) => (r.out.branch(name), false),
                    None => match c.parents.get(k) {
                        Some(&parent) => (r.out.unnamed(parent), false),
                        None => continue,
                    },
                };
                r.set(j, o, by, p);
                r.meet(j, o);
            }
        }
        if g.bottom.iter().any(|&(_, j)| j == g.col) || c.parents.is_empty() {
            r.meet(g.col, node);
        }
        for &(l, _) in &g.top {
            if r.owner(l).is_none()
                && let Some(runs) = r.out.runs.get_mut(l)
                && runs.last().is_some_and(|run| run.1 != ENDED)
            {
                runs.push((p.0, ENDED));
            }
        }
    }
    let mut out = r.out;
    let mut pairs = r.pairs;
    pairs.sort_unstable();
    pairs.dedup();
    out.neighbours = pairs;
    out
}

#[cfg(test)]
#[path = "owners_tests.rs"]
mod tests;

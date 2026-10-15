//! Selecting several commits in the graph (Ctrl- and Shift-click). Pure logic: `update`
//! decides which effects a new selection needs.

use super::Selection;
use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::{Comparison, Snapshot};

pub const PICK_BLOCKED_MERGE: &str = "Merge commits can't be cherry-picked from a multi-selection.";
pub const PICK_BLOCKED_ON_HEAD: &str = "Some selected commits are already on the current branch.";

#[derive(Clone, PartialEq, Debug)]
pub struct MultiSelection {
    /// Selected commits in graph order (newest first), at least two, no duplicates.
    pub oids: Vec<Oid>,
    /// Where a Shift range starts: the last Ctrl-clicked commit, or the single commit selected
    /// before.
    pub anchor: Oid,
    /// With exactly two commits: the files that differ between them and which are missing from
    /// the working tree; `None` while loading.
    pub changes: Option<Result<Comparison, String>>,
    /// Why "Cherry-pick N commits" is unavailable (merges, or commits already on HEAD).
    pub pick_blocked: Option<&'static str>,
    /// Why "Create patch from N commits" is unavailable (merges).
    pub export_blocked: Option<&'static str>,
}

impl MultiSelection {
    /// The selection made of `oids` (those that are graph rows, in graph order): none, one
    /// commit, or a multi-selection with nothing loaded yet.
    pub fn build(oids: impl IntoIterator<Item = Oid>, anchor: Oid, snap: &Snapshot) -> Selection {
        let oids = in_graph_order(oids, snap, false);
        match oids.as_slice() {
            [] => Selection::None,
            [one] => Selection::Commit(*one),
            _ => {
                let pick_blocked = cherry_pick_blocker(snap, &oids);
                let export_blocked = super::patch::export_blocker(snap, &oids);
                Selection::Commits(Self { oids, anchor, changes: None, pick_blocked, export_blocked })
            }
        }
    }

    pub fn contains(&self, oid: Oid) -> bool {
        self.oids.contains(&oid)
    }

    pub fn len(&self) -> usize {
        self.oids.len()
    }

    /// With exactly two commits: (older, newer), i.e. the lower row first.
    pub fn pair(&self) -> Option<(Oid, Oid)> {
        match self.oids.as_slice() {
            [newer, older] => Some((*older, *newer)),
            _ => None,
        }
    }

    /// The commit keyboard navigation and Esc start from: the anchor while it is selected,
    /// otherwise the newest selected commit.
    pub fn focus(&self) -> Oid {
        if self.contains(self.anchor) { self.anchor } else { self.oids[0] }
    }
}

/// The graph rows among `oids`, without duplicates, newest first (or oldest first with `rev`).
pub fn in_graph_order(oids: impl IntoIterator<Item = Oid>, snap: &Snapshot, rev: bool) -> Vec<Oid> {
    let mut rows: Vec<usize> = oids.into_iter().filter_map(|o| snap.row_of.get(&o).copied()).collect();
    rows.sort_unstable();
    rows.dedup();
    if rev {
        rows.reverse();
    }
    rows.into_iter().map(|r| snap.commits[r].oid).collect()
}

/// The selection after a Ctrl-click on `oid`: it is added, or removed if it was selected, and
/// becomes the anchor. A toggle never empties the selection.
pub fn toggled(sel: &Selection, oid: Oid, snap: &Snapshot) -> Selection {
    if !snap.row_of.contains_key(&oid) {
        return sel.clone();
    }
    match sel {
        Selection::None | Selection::Wip => Selection::Commit(oid),
        Selection::Commit(a) if *a == oid => sel.clone(),
        Selection::Commit(a) => MultiSelection::build([*a, oid], oid, snap),
        Selection::Commits(m) => {
            let mut oids = m.oids.clone();
            if m.contains(oid) {
                oids.retain(|&o| o != oid);
            } else {
                oids.push(oid);
            }
            MultiSelection::build(oids, oid, snap)
        }
    }
}

/// The selection after a Shift-click on `oid`: every graph row from the anchor to it. While the
/// graph filters (`shown`: the commit indexes it shows, ascending), only the shown ones.
pub fn ranged(sel: &Selection, oid: Oid, snap: &Snapshot, shown: Option<&[usize]>) -> Selection {
    let Some(&to) = snap.row_of.get(&oid) else { return sel.clone() };
    let anchor = match sel {
        Selection::Commit(a) => *a,
        Selection::Commits(m) => m.anchor,
        Selection::None | Selection::Wip => return Selection::Commit(oid),
    };
    let Some(&from) = snap.row_of.get(&anchor) else { return Selection::Commit(oid) };
    let (lo, hi) = (from.min(to), from.max(to));
    let in_range = (lo..=hi).filter(|i| shown.is_none_or(|s| s.binary_search(i).is_ok()));
    MultiSelection::build(in_range.filter_map(|i| snap.commits.get(i)).map(|c| c.oid), anchor, snap)
}

/// Why these commits can't be cherry-picked together onto HEAD, if they can't.
pub fn cherry_pick_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<&'static str> {
    let commit = |o: &Oid| snap.row_of.get(o).map(|&r| &snap.commits[r]);
    if oids.iter().filter_map(commit).any(|c| c.parents.len() > 1) {
        return Some(PICK_BLOCKED_MERGE);
    }
    let head = snap.head_oid?;
    let on_head: HashSet<Oid> = snap.commits_between(head, None).iter().map(|c| c.oid).collect();
    oids.iter().any(|o| on_head.contains(o)).then_some(PICK_BLOCKED_ON_HEAD)
}

/// `sel` against a newly loaded snapshot: commits that are gone are dropped, the rest re-sorted.
/// A file list already received for the same pair is kept (commits don't change).
pub fn refreshed(sel: &MultiSelection, snap: &Snapshot) -> Selection {
    let mut new = MultiSelection::build(sel.oids.iter().copied(), sel.anchor, snap);
    if let Selection::Commits(m) = &mut new
        && m.pair().is_some()
        && m.pair() == sel.pair()
    {
        m.changes = sel.changes.clone();
    }
    new
}

//! Selecting several commits in the graph (Ctrl- and Shift-click, Shift+arrows, select all).
//! Pure logic: `update` decides which effects a new selection needs.

use super::Selection;
use super::selection_span::{Span, span_of};
use super::selection_wip::WorkdirCompare;
use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::{Comparison, Snapshot};

pub const PICK_BLOCKED_MERGE: &str = "Merge commits can't be cherry-picked from a multi-selection.";
pub const PICK_BLOCKED_ON_HEAD: &str = "Some selected commits are already on the current branch.";
/// Why "Cherry-pick commit" is unavailable for one commit in HEAD's history.
pub const PICK_ONE_ON_HEAD: &str = "This commit is already on the current branch.";
pub const REVERT_BLOCKED_MERGE: &str = "Merge commits can't be reverted from a multi-selection.";

#[derive(Clone, PartialEq, Debug)]
pub struct MultiSelection {
    /// Selected commits in graph order (newest first), at least two, no duplicates.
    pub oids: Vec<Oid>,
    /// Where a Shift range starts: the last Ctrl-clicked commit, or the single commit selected
    /// before.
    pub anchor: Oid,
    /// The moving end: the commit of the last Shift range or Ctrl-click, which Shift+Up and
    /// Shift+Down extend from.
    pub cursor: Oid,
    /// `oids` as a set, for `contains` on every graph and sidebar row.
    pub members: HashSet<Oid>,
    /// A pair, one line of history, or neither: whether there is a file list, and of what.
    pub span: Span,
    /// The comparison runs the other way, as Swap direction asked.
    pub swapped: bool,
    /// The files that differ between `compare_ends` and which are missing from the working
    /// tree; `None` while loading (and always without ends).
    pub changes: Option<Result<Comparison, String>>,
    /// Why "Cherry-pick N commits" is unavailable (merges, or commits already on HEAD).
    pub pick_blocked: Option<&'static str>,
    /// Why "Revert N commits" is unavailable (merges).
    pub revert_blocked: Option<&'static str>,
    /// Why "Create patch from N commits" is unavailable (merges).
    pub export_blocked: Option<&'static str>,
}

impl MultiSelection {
    /// The selection made of `oids` (those that are graph rows, in graph order): none, one
    /// commit, or a multi-selection with nothing loaded yet. The cursor is the anchor.
    pub fn build(oids: impl IntoIterator<Item = Oid>, anchor: Oid, snap: &Snapshot) -> Selection {
        Self::build_at(oids, anchor, anchor, snap)
    }

    /// `build` with the moving end at `cursor`.
    pub fn build_at(oids: impl IntoIterator<Item = Oid>, anchor: Oid, cursor: Oid, snap: &Snapshot) -> Selection {
        let oids = in_graph_order(oids, snap, false);
        match oids.as_slice() {
            [] => Selection::None,
            [one] => Selection::Commit(*one),
            _ => {
                let pick_blocked = cherry_pick_blocker(snap, &oids);
                let revert_blocked = revert_blocker(snap, &oids);
                let export_blocked = super::patch::export_blocker(snap, &oids);
                let members = oids.iter().copied().collect();
                let span = span_of(&oids, snap);
                Selection::Commits(Self {
                    oids,
                    anchor,
                    cursor,
                    members,
                    span,
                    swapped: false,
                    changes: None,
                    pick_blocked,
                    revert_blocked,
                    export_blocked,
                })
            }
        }
    }

    pub fn contains(&self, oid: Oid) -> bool {
        self.members.contains(&oid)
    }

    pub fn len(&self) -> usize {
        self.oids.len()
    }

    /// The comparison the panel shows, (from, to): a pair older to newer, a line of history
    /// from the oldest commit's parent to the newest; reversed when swapped. `None` for
    /// commits that aren't one line.
    pub fn compare_ends(&self) -> Option<(Oid, Oid)> {
        let (from, to) = self.span.ends()?;
        Some(if self.swapped { (to, from) } else { (from, to) })
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
/// becomes the anchor and cursor. With the WIP row selected, the commit is compared with the
/// working directory. A toggle never empties the selection.
pub fn toggled(sel: &Selection, oid: Oid, snap: &Snapshot) -> Selection {
    if !snap.row_of.contains_key(&oid) {
        return sel.clone();
    }
    match sel {
        Selection::None => Selection::Commit(oid),
        Selection::Wip => Selection::VsWorkdir(WorkdirCompare::new(oid)),
        Selection::VsWorkdir(w) if w.oid == oid => Selection::Wip,
        Selection::VsWorkdir(w) => MultiSelection::build([w.oid, oid], oid, snap),
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

/// The commit Shift+Up and Shift+Down extend from: the one commit (compared or not), or a
/// multi-selection's cursor while it is selected (else its focus). `None` without a commit.
pub fn extend_from(sel: &Selection) -> Option<Oid> {
    match sel {
        Selection::Commit(a) => Some(*a),
        Selection::VsWorkdir(w) => Some(w.oid),
        Selection::Commits(m) => Some(if m.contains(m.cursor) { m.cursor } else { m.focus() }),
        Selection::None | Selection::Wip => None,
    }
}

/// The selection after a Shift-click on `oid`: every graph row from the anchor to it. While the
/// graph filters (`shown`: the commit indexes it shows, ascending), only the shown ones.
pub fn ranged(sel: &Selection, oid: Oid, snap: &Snapshot, shown: Option<&[usize]>) -> Selection {
    let Some(&to) = snap.row_of.get(&oid) else { return sel.clone() };
    let anchor = match sel {
        Selection::Commit(a) => *a,
        Selection::VsWorkdir(w) => w.oid,
        Selection::Commits(m) => m.anchor,
        Selection::None | Selection::Wip => return Selection::Commit(oid),
    };
    let Some(&from) = snap.row_of.get(&anchor) else { return Selection::Commit(oid) };
    let (lo, hi) = (from.min(to), from.max(to));
    let in_range = (lo..=hi).filter(|i| shown.is_none_or(|s| s.binary_search(i).is_ok()));
    MultiSelection::build_at(in_range.filter_map(|i| snap.commits.get(i)).map(|c| c.oid), anchor, oid, snap)
}

fn is_merge(snap: &Snapshot, oid: &Oid) -> bool {
    snap.row_of.get(oid).is_some_and(|&r| snap.commits[r].parents.len() > 1)
}

/// Why these commits can't be cherry-picked onto HEAD, if they can't: merges among several
/// (one merge goes to the mainline dialog), or commits already in HEAD's history.
pub fn cherry_pick_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<&'static str> {
    if oids.len() > 1 && oids.iter().any(|o| is_merge(snap, o)) {
        return Some(PICK_BLOCKED_MERGE);
    }
    let head = snap.head_oid?;
    if let [one] = oids {
        return on_head(snap, head, *one).then_some(PICK_ONE_ON_HEAD);
    }
    let on_head: HashSet<Oid> = snap.commits_between(head, None).iter().map(|c| c.oid).collect();
    oids.iter().any(|o| on_head.contains(o)).then_some(PICK_BLOCKED_ON_HEAD)
}

/// Whether `oid` is in `head`'s loaded history. The graph is in topological order (children
/// first), so only the rows between the two can lead from one to the other.
fn on_head(snap: &Snapshot, head: Oid, oid: Oid) -> bool {
    let (Some(&top), Some(&target)) = (snap.row_of.get(&head), snap.row_of.get(&oid)) else { return false };
    if target < top {
        return false;
    }
    let mut seen = HashSet::new();
    let mut stack = vec![top];
    while let Some(r) = stack.pop() {
        if r == target {
            return true;
        }
        if seen.insert(r) {
            let parents = snap.commits[r].parents.iter().filter_map(|p| snap.row_of.get(p).copied());
            stack.extend(parents.filter(|&p| p <= target));
        }
    }
    false
}

/// Why these commits can't be reverted together, if they can't: merges among several (one
/// merge goes to the mainline dialog).
pub fn revert_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<&'static str> {
    (oids.len() > 1 && oids.iter().any(|o| is_merge(snap, o))).then_some(REVERT_BLOCKED_MERGE)
}

/// `sel` against a newly loaded snapshot: commits that are gone are dropped, the rest re-sorted.
/// While it compares the same ends, the direction and a file list already received are kept
/// (commits don't change).
pub fn refreshed(sel: &MultiSelection, snap: &Snapshot) -> Selection {
    let mut new = MultiSelection::build_at(sel.oids.iter().copied(), sel.anchor, sel.cursor, snap);
    if let Selection::Commits(m) = &mut new
        && m.span.ends().is_some()
        && m.span.ends() == sel.span.ends()
    {
        m.swapped = sel.swapped;
        m.changes = sel.changes.clone();
    }
    new
}

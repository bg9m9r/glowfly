//! Selecting several commits in the graph (Ctrl- and Shift-click, Shift+arrows, select all).
//! Pure logic: `update` decides which effects a new selection needs.

use super::Selection;
use super::selection_span::{Span, span_of};
use super::selection_wip::WorkdirCompare;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Comparison, Snapshot, StackedCommit};
use std::collections::HashSet;

/// The most commits a stacked list loads the changes of (the newest selected).
pub const STACK_LIMIT: usize = 200;

/// Why a commit action is unavailable for the selected commits. Kept as a value and put into
/// words when drawn or logged, so a language switch needs no reselect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitBlock {
    /// Some of several commits to cherry-pick are already on HEAD.
    PickOnHead,
    /// The one commit to cherry-pick is already in HEAD's history.
    PickOneOnHead,
    /// A merge commit has no patch of its own.
    PatchMerge,
}

impl CommitBlock {
    pub fn text(self, t: Lang) -> &'static str {
        match self {
            CommitBlock::PickOnHead => t.tr("commits.blocked.pick_on_head"),
            CommitBlock::PickOneOnHead => t.tr("commits.blocked.pick_one_on_head"),
            CommitBlock::PatchMerge => t.tr("commits.blocked.patch_merge"),
        }
    }
}

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
    /// A pair, one line of history, or neither: one combined file list, or a stacked list of
    /// each commit's own changes.
    pub span: Span,
    /// The comparison runs the other way, as Swap direction asked.
    pub swapped: bool,
    /// The files that differ between `compare_ends` and which are missing from the working
    /// tree; `None` while loading (and always without ends).
    pub changes: Option<Result<Comparison, String>>,
    /// Each of `stack_oids`' own changes, newest first; `None` while loading (and always for a
    /// pair or a line of history).
    pub stacked: Option<Result<Vec<StackedCommit>, String>>,
    /// Why "Cherry-pick N commits" is unavailable (commits already on HEAD).
    pub pick_blocked: Option<CommitBlock>,
    /// Why "Create patch from N commits" is unavailable (merges).
    pub export_blocked: Option<CommitBlock>,
}

impl MultiSelection {
    /// The selection made of `oids` (those that are graph rows, in graph order): none, one
    /// commit, or a multi-selection with nothing loaded yet. The cursor is the anchor.
    pub fn build(oids: impl IntoIterator<Item = Oid>, anchor: Oid, snap: &Snapshot) -> Selection {
        Self::build_at(oids, anchor, anchor, snap)
    }

    /// `build` with the moving end at `cursor`.
    fn build_at(oids: impl IntoIterator<Item = Oid>, anchor: Oid, cursor: Oid, snap: &Snapshot) -> Selection {
        let oids = in_graph_order(oids, snap, false);
        match oids.as_slice() {
            [] => Selection::None,
            [one] => Selection::Commit(*one),
            _ => {
                let pick_blocked = cherry_pick_blocker(snap, &oids);
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
                    stacked: None,
                    pick_blocked,
                    export_blocked,
                })
            }
        }
    }

    pub fn contains(&self, oid: Oid) -> bool {
        self.members.contains(&oid)
    }

    // Always two commits or more (fewer is `Selection::None` or `Selection::Commit`): no `is_empty`.
    #[allow(clippy::len_without_is_empty)]
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

    /// The commits whose own changes the stacked list shows: the newest `STACK_LIMIT` selected,
    /// when there is no combined comparison.
    pub fn stack_oids(&self) -> Option<&[Oid]> {
        self.compare_ends().is_none().then(|| stack_slice(&self.oids))
    }

    /// The commit named as the reason there is no combined diff (`Span::Gaps`).
    pub fn gap(&self) -> Option<Oid> {
        match self.span {
            Span::Gaps { gap } => gap,
            _ => None,
        }
    }

    /// The commit keyboard navigation and Esc start from: the anchor while it is selected,
    /// otherwise the newest selected commit.
    pub fn focus(&self) -> Oid {
        if self.contains(self.anchor) { self.anchor } else { self.oids[0] }
    }
}

/// The newest `STACK_LIMIT` of `oids` (newest first).
pub fn stack_slice(oids: &[Oid]) -> &[Oid] {
    &oids[..oids.len().min(STACK_LIMIT)]
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

/// Why these commits can't be cherry-picked onto HEAD, if they can't: commits already in
/// HEAD's history. (Merges go to the mainline dialog first.)
pub fn cherry_pick_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<CommitBlock> {
    let head = snap.head_oid?;
    if let [one] = oids {
        return on_head(snap, head, *one).then_some(CommitBlock::PickOneOnHead);
    }
    let on_head: HashSet<Oid> = snap.commits_between(head, None).iter().map(|c| c.oid).collect();
    oids.iter().any(|o| on_head.contains(o)).then_some(CommitBlock::PickOnHead)
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

/// `sel` against a newly loaded snapshot: commits that are gone are dropped, the rest re-sorted.
/// While it compares the same ends, the direction and a file list already received are kept
/// (commits don't change); so is a stacked list of the same commits.
pub fn refreshed(sel: &MultiSelection, snap: &Snapshot) -> Selection {
    let mut new = MultiSelection::build_at(sel.oids.iter().copied(), sel.anchor, sel.cursor, snap);
    if let Selection::Commits(m) = &mut new {
        if m.span.ends().is_some() && m.span.ends() == sel.span.ends() {
            m.swapped = sel.swapped;
            m.changes = sel.changes.clone();
        }
        if m.stack_oids().is_some() && m.stack_oids() == sel.stack_oids() {
            m.stacked = sel.stacked.clone();
        }
    }
    new
}

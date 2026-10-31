//! Which graph rows belong to a branch, for highlighting the rows of the ref under the pointer.
//! Pure walks over the loaded history; `update` runs them (`GraphMsg::HoverRef`, the default
//! branch's ancestry on a worker for a large history) and keeps them per snapshot in `RefRows`;
//! views only read `ViewState::highlight`.

use git2::Oid;
use glowfly_core::repo::{CommitInfo, Snapshot};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The walks for the snapshot on screen, done once each (update/graph_filter.rs).
#[derive(Default)]
pub struct RefRows {
    /// The commits reachable from the default branch, walked on the first hover.
    pub base: Option<HashSet<Oid>>,
    /// The request walking it on a worker, if any.
    pub asked: Option<u64>,
    /// Counts those requests; kept by `reset`, so an answer for an older snapshot can't pass.
    pub requests: u64,
    /// Each hovered tip's branch rows.
    pub rows: HashMap<Oid, Arc<HashSet<Oid>>>,
}

impl RefRows {
    /// A new snapshot or tab: every walk is done again when next needed.
    pub fn reset(&mut self) {
        *self = RefRows { requests: self.requests, ..RefRows::default() };
    }
}

/// Commits that belong to the branch whose tip is `tip`: its first-parent chain, down to where
/// it joins the default branch's history.
pub fn branch_rows(snap: &Snapshot, tip: Oid, base: &HashSet<Oid>) -> HashSet<Oid> {
    let mut out = HashSet::new();
    let on_base = base.contains(&tip);
    let mut cur = Some(tip);
    while let Some(oid) = cur {
        if !on_base && base.contains(&oid) {
            break;
        }
        let Some(&row) = snap.row_of.get(&oid) else { break };
        out.insert(oid);
        cur = snap.commits[row].parents.first().copied();
        if out.len() > 5000 {
            break;
        }
    }
    out
}

/// All commits reachable (within the loaded history) from the default branch tip.
pub fn default_ancestry(snap: &Snapshot) -> (Option<Oid>, HashSet<Oid>) {
    let tip = default_tip(snap);
    (tip, reachable(&snap.commits, |o| snap.row_of.get(o).copied(), tip))
}

/// The default branch's tip: main, master, develop or trunk, else HEAD.
pub fn default_tip(snap: &Snapshot) -> Option<Oid> {
    ["main", "master", "develop", "trunk"]
        .iter()
        .find_map(|n| snap.locals.iter().find(|b| b.name == *n).and_then(|b| b.oid))
        .or(snap.head_oid)
}

/// The commits of `commits` reachable from `tip`, on a worker: without the snapshot's row map,
/// it makes its own.
pub fn reachable_in(commits: &[CommitInfo], tip: Option<Oid>) -> HashSet<Oid> {
    let row_of: HashMap<Oid, usize> = commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();
    reachable(commits, |o| row_of.get(o).copied(), tip)
}

/// The commits reachable from `tip` among `commits`, whose rows `row` finds.
fn reachable(commits: &[CommitInfo], row: impl Fn(&Oid) -> Option<usize>, tip: Option<Oid>) -> HashSet<Oid> {
    let mut seen = HashSet::new();
    let mut stack: Vec<Oid> = tip.into_iter().collect();
    while let Some(o) = stack.pop() {
        if !seen.insert(o) {
            continue;
        }
        if let Some(r) = row(&o) {
            stack.extend(commits[r].parents.iter().copied());
        }
    }
    seen
}

//! Which graph rows belong to a branch, for highlighting the rows of the ref under the pointer.
//! Pure walks over the loaded history; `update` runs them (`GraphMsg::HoverRef`) and views only read
//! `ViewState::highlight`.

use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::Snapshot;

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
    let tip = ["main", "master", "develop", "trunk"]
        .iter()
        .find_map(|n| snap.locals.iter().find(|b| b.name == *n).and_then(|b| b.oid))
        .or(snap.head_oid);
    let mut seen = HashSet::new();
    let mut stack: Vec<Oid> = tip.into_iter().collect();
    while let Some(o) = stack.pop() {
        if !seen.insert(o) {
            continue;
        }
        if let Some(&row) = snap.row_of.get(&o) {
            stack.extend(snap.commits[row].parents.iter().copied());
        }
    }
    (tip, seen)
}

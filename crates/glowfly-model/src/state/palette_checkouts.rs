//! The palette's checkout entries: local, remote and tag checkouts. They are the entries that
//! grow with the repository (one per ref), built from the refs alone so the palette's cache
//! (`palette_cache.rs`) can keep them until the refs change.

use super::{Entry, PaletteCommand, entry, with_detail};
use glowfly_core::repo::Snapshot;
use std::collections::HashSet;

/// The checkout entries of a snapshot: `locals` go among the other commands, `others` (remote
/// branch and tag checkouts) after them all.
#[derive(Default)]
pub(crate) struct RefEntries {
    pub(crate) locals: Vec<Entry>,
    pub(crate) others: Vec<Entry>,
}

/// The checkout entries for `snap` (none without a repository). They read only the snapshot's
/// `locals`, `worktrees`, `remotes`, `tags`, `head_branch` and `head_oid`.
pub(crate) fn ref_entries(snap: Option<&Snapshot>) -> RefEntries {
    let mut refs = RefEntries::default();
    if let Some(snap) = snap {
        checkout_entries(snap, &mut refs.locals);
        remote_checkout_entries(snap, &mut refs.others);
        tag_checkout_entries(snap, &mut refs.others);
    }
    refs
}

/// A checkout entry for each local branch other than HEAD's and those checked out in another
/// worktree (those are offered as Open worktree instead).
fn checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let elsewhere = |name: &str| crate::state::worktree_branches::elsewhere(snap, name).is_some();
    for b in snap.locals.iter().filter(|b| !b.is_head && !elsewhere(&b.name)) {
        let e = entry(PaletteCommand::Checkout(b.name.clone()), format!("Checkout {}", b.name), "switch branch");
        out.push(with_detail(e, b.upstream.clone()));
    }
}

/// A checkout entry for each remote branch that no local branch shares a name with or tracks
/// (its configured upstream counts even when the ref is gone): checking one out creates a local
/// branch tracking it.
fn remote_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let local: HashSet<&str> = snap.locals.iter().map(|b| b.name.as_str()).collect();
    let mut tracked: HashSet<String> = HashSet::new();
    for b in &snap.locals {
        tracked.extend(b.upstream.clone());
        if let Some(t) = b.tracking.as_ref().filter(|t| t.is_remote()) {
            tracked.insert(t.display());
        }
    }
    for (remote, branches) in &snap.remotes {
        for (branch, _) in branches {
            if branch == "HEAD" || local.contains(branch.as_str()) || tracked.contains(&format!("{remote}/{branch}")) {
                continue;
            }
            let command = PaletteCommand::CheckoutRemote { remote: remote.clone(), branch: branch.clone() };
            let e = entry(command, format!("Checkout {remote}/{branch}"), "switch branch remote track tracking");
            out.push(with_detail(e, Some("creates local branch".to_owned())));
        }
    }
}

/// Hidden words for tag checkouts. "detached" comes first: after "release" its letters would
/// let a query such as "create" match every tag.
const TAG_KEYWORDS: &str = "detached switch tag release version";

/// A checkout entry for each tag, except one at the commit HEAD is already detached at.
fn tag_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let detached_at = if snap.head_branch.is_none() { snap.head_oid } else { None };
    for (name, oid) in &snap.tags {
        if Some(*oid) == detached_at {
            continue;
        }
        let e = entry(PaletteCommand::CheckoutTag(name.clone()), format!("Checkout tag {name}"), TAG_KEYWORDS);
        out.push(with_detail(e, Some(format!("detached at {}", &oid.to_string()[..7]))));
    }
}

//! The palette's checkout and recent-repository entries: local, remote and tag checkouts, and
//! the recent repositories other than the active one.

use super::{Entry, PaletteCommand, entry, folder_name, with_detail};
use crate::state::AppState;
use std::collections::HashSet;
use trellis_core::repo::Snapshot;

/// Recent repositories offered by the palette.
const MAX_RECENT: usize = 10;

/// A checkout entry for each local branch other than HEAD's.
pub(super) fn checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    for b in snap.locals.iter().filter(|b| !b.is_head) {
        let e = entry(PaletteCommand::Checkout(b.name.clone()), format!("Checkout {}", b.name), "switch branch");
        out.push(with_detail(e, b.upstream.clone()));
    }
}

/// A checkout entry for each remote branch that no local branch shares a name with or tracks
/// (its configured upstream counts even when the ref is gone): checking one out creates a local
/// branch tracking it.
pub(super) fn remote_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
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
pub(super) fn tag_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let detached_at = if snap.head_branch.is_none() { snap.head_oid } else { None };
    for (name, oid) in &snap.tags {
        if Some(*oid) == detached_at {
            continue;
        }
        let e = entry(PaletteCommand::CheckoutTag(name.clone()), format!("Checkout tag {name}"), TAG_KEYWORDS);
        out.push(with_detail(e, Some(format!("detached at {}", &oid.to_string()[..7]))));
    }
}

/// Recent repositories other than the active one.
pub(super) fn recent_entries(st: &AppState, out: &mut Vec<Entry>) {
    let active = st.workdir();
    for path in st.recent.iter().take(MAX_RECENT) {
        let is_active =
            active.as_deref().is_some_and(|w| st.same_folder(w, path)) || st.snap.as_ref().is_some_and(|s| st.names_repo(s, path));
        if is_active || crate::state::repo_lists::is_missing(st, path) {
            continue;
        }
        let e = entry(PaletteCommand::OpenRecent(path.clone()), format!("Open {}", folder_name(path)), "recent repository");
        out.push(with_detail(e, Some(trellis_core::native_path::shown(path))));
    }
}

//! The palette's entries for a shallow clone: fetch its full history, or a number of older
//! commits (a free-text step).

use super::{Entry, PaletteCommand, entry, with_detail};
use crate::state::shallow::history_remote;
use glowfly_core::repo::Snapshot;

/// Fetch full history and Fetch more commits…, only in a shallow clone with a remote.
pub(super) fn shallow_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if !snap.is_shallow() {
        return;
    }
    let Some(remote) = history_remote(snap) else { return };
    let from = format!("from {remote}");
    let full = entry(
        PaletteCommand::FetchFullHistory,
        "Fetch full history",
        "unshallow shallow clone complete whole entire all history older commits download",
    );
    out.push(with_detail(full, Some(from.clone())));
    let more = entry(
        PaletteCommand::FetchMoreCommits,
        "Fetch more commits…",
        "deepen shallow clone depth number count older history commits download",
    );
    out.push(with_detail(more, Some(from)));
}

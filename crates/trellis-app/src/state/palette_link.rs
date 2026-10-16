//! The palette's entries for copying a trellis:// link to the open repository and to the
//! checked-out branch.

use super::{Entry, PaletteCommand, entry};
use crate::deep_link::LinkTarget;
use trellis_core::repo::Snapshot;

/// Copy link to repository, and Copy link to branch <head> when HEAD is on a branch. Only for a
/// repository a link can name: one with a commit or a remote.
pub(super) fn link_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if snap.head_oid.is_none() && snap.remote_urls.is_empty() {
        return;
    }
    const WORDS: &str = "deep link url share trellis";
    out.push(entry(PaletteCommand::CopyLink(LinkTarget::Repository { remote: None }), "Copy link to repository", WORDS));
    if let Some(b) = &snap.head_branch {
        out.push(entry(PaletteCommand::CopyLink(LinkTarget::LocalBranch(b.clone())), format!("Copy link to branch {b}"), WORDS));
    }
}

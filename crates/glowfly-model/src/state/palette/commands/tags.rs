//! The palette's tag entries: Delete tag…, whose sub-list of the repository's tags opens the
//! Delete tag dialog (only while there are tags).

use super::{Entry, PaletteCommand as C, entry};
use glowfly_core::repo::Snapshot;

pub(super) fn tag_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if !snap.tags.is_empty() {
        out.push(entry(C::ChooseTagToDelete, "Delete tag…", "remove tag release version remote delete"));
    }
}

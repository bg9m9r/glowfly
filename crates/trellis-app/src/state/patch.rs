//! Saving commits as a patch file and applying one: why an export is unavailable, the suggested
//! file name, the result toast, the pending apply job's headline data and the palette entry.
//! Pure.

use super::palette_commands::{Entry, PaletteCommand, entry};
use git2::Oid;
use std::path::Path;
use trellis_core::patch_ops::{self, ExportReport};
use trellis_core::repo::Snapshot;

/// The job label of applying a patch.
pub const APPLY_LABEL: &str = "Apply patch";
/// Why Create patch is unavailable for a selection with a merge commit.
pub const PATCH_BLOCKED_MERGE: &str = "Merge commits can't be saved as a patch.";

/// Why `oids` can't be saved as a patch, if they can't: format-patch has no diff for a merge.
pub fn export_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<&'static str> {
    let merge = oids.iter().filter_map(|o| snap.row_of.get(o).map(|&r| &snap.commits[r])).any(|c| c.parents.len() > 1);
    merge.then_some(PATCH_BLOCKED_MERGE)
}

/// The suggested file name for a patch of `oldest_first`, from the oldest commit's subject.
pub fn default_name(snap: &Snapshot, oldest_first: &[Oid]) -> Option<String> {
    let oldest = snap.row_of.get(oldest_first.first()?).map(|&r| &snap.commits[r])?;
    Some(patch_ops::file_name(&oldest.summary, oldest_first.len()))
}

/// A file's own name, or the whole path when it has none.
pub fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// The toast after saving a patch into `dest`.
pub fn export_done(dest: &Path, r: &ExportReport) -> String {
    let name = display_name(dest);
    let mut text = if r.written == 1 { format!("Saved the patch to {name}") } else { format!("Saved {} commits to {name}", r.written) };
    match r.skipped.len() {
        0 => {}
        1 => text.push_str(" (1 commit with no changes left out)"),
        k => text.push_str(&format!(" ({k} commits with no changes left out)")),
    }
    text
}

/// The running Apply patch job, for its headline.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingApply {
    /// The job's label ([`APPLY_LABEL`]).
    pub label: String,
    /// The patch's file name.
    pub name: String,
}

/// The palette's Apply patch entry, offered whenever a repository is open.
pub(super) fn palette_entries(out: &mut Vec<Entry>) {
    out.push(entry(PaletteCommand::ApplyPatch, "Apply patch…", "patch diff apply import mbox file changes working tree"));
}

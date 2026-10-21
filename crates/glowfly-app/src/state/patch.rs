//! Patch files: why an export is unavailable, the suggested file name, the save toast, the
//! pending apply job's headline data and the palette entries. The Apply patch dialog's form and
//! texts are in `patch_apply.rs`, the Create patch dialog's (uncommitted changes) in
//! `patch_changes.rs`. Pure.

#[path = "patch_apply.rs"]
pub mod apply;
#[path = "patch_changes.rs"]
pub mod changes;

use super::AppState;
use super::Selection;
use super::palette_commands::{Entry, PaletteCommand, entry};
use super::selection::CommitBlock;
use crate::patch::{ApplyHow, PatchMsg};
use git2::Oid;
use glowfly_core::patch_ops::{self, SaveReport, WipScope};
use glowfly_core::repo::Snapshot;
use std::path::Path;

/// The job label of applying a patch's changes.
pub const APPLY_LABEL: &str = "Apply patch";
/// The job label of applying a patch as commits.
pub const AM_LABEL: &str = "Apply patches";

/// Why `oids` can't be saved as a patch, if they can't: format-patch has no diff for a merge.
pub fn export_blocker(snap: &Snapshot, oids: &[Oid]) -> Option<CommitBlock> {
    let merge = oids.iter().filter_map(|o| snap.row_of.get(o).map(|&r| &snap.commits[r])).any(|c| c.parents.len() > 1);
    merge.then_some(CommitBlock::PatchMerge)
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
pub fn export_done(dest: &Path, r: &SaveReport) -> String {
    let name = display_name(dest);
    let r = match r {
        SaveReport::Commits(r) => r,
        SaveReport::Changes { files: 1 } => return format!("Saved the changes to {name} (1 file)"),
        SaveReport::Changes { files } => return format!("Saved the changes to {name} ({files} files)"),
    };
    let mut text = if r.written == 1 { format!("Saved the patch to {name}") } else { format!("Saved {} commits to {name}", r.written) };
    match r.skipped.len() {
        0 => {}
        1 => text.push_str(" (1 commit with no changes left out)"),
        k => text.push_str(&format!(" ({k} commits with no changes left out)")),
    }
    text
}

/// The running Apply patch or Apply patches job, for its headline.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingApply {
    /// The job's label ([`APPLY_LABEL`] or [`AM_LABEL`]).
    pub label: String,
    /// The patch's file name.
    pub name: String,
    pub how: ApplyHow,
    /// Emailed commits in the file.
    pub commits: usize,
}

/// The palette's patch entries: Apply patch… whenever a repository is open, Create patch from
/// the uncommitted changes while there are any, and Create or Copy a patch of the selected
/// commit(s) unless a merge is among them.
pub(super) fn palette_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    out.push(entry(PaletteCommand::ApplyPatch, "Apply patch…", "patch diff apply import mbox file changes working tree"));
    if snap.is_dirty() {
        out.push(entry(
            PaletteCommand::Patch(PatchMsg::ChangesDialog { scope: WipScope::All, path: None }),
            "Create patch from uncommitted changes…",
            "patch diff export save uncommitted changes wip working tree",
        ));
    }
    match &st.selection {
        Selection::Commits(m) if m.export_blocked.is_none() => out.push(entry(
            PaletteCommand::Patch(PatchMsg::Export(m.oids.clone())),
            format!("Create patch from {} selected commits…", m.len()),
            "patch diff export save format-patch selection",
        )),
        Selection::Commit(oid) if export_blocker(snap, &[*oid]).is_none() && snap.row_of.contains_key(oid) => {
            let short = &oid.to_string()[..7];
            out.push(entry(
                PaletteCommand::Patch(PatchMsg::Export(vec![*oid])),
                format!("Create patch from commit {short}…"),
                "patch diff export save format-patch",
            ));
            out.push(entry(
                PaletteCommand::Patch(PatchMsg::Copy(vec![*oid])),
                format!("Copy patch of commit {short}"),
                "patch diff clipboard",
            ));
        }
        _ => {}
    }
}

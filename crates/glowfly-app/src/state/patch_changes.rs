//! The Create patch dialog for uncommitted changes (`Modal::SavePatch`): which changes go in
//! (all, staged or unstaged), whether untracked files do, the files ticked under Only selected
//! files, why it can't be saved now, and the request it makes. The list is built from the
//! snapshot every frame; ticks are kept by path. Pure.

use super::super::stashes::wip_files;
use glowfly_core::patch_ops::{ChangesRequest, NO_CHANGES, WipScope};
use glowfly_core::repo::{FileEntry, Snapshot};
use std::collections::BTreeSet;

pub const RESOLVE_FIRST: &str = "Resolve the conflicted files first.";
pub const NOTHING_STAGED: &str = "Nothing is staged.";
pub const NO_PICK: &str = "Choose at least one file.";

/// The Create patch dialog's form.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveChangesForm {
    pub scope: WipScope,
    /// Only selected files: just the paths in `chosen`.
    pub pick: bool,
    /// Paths ticked under Only selected files, kept across refreshes.
    pub chosen: BTreeSet<String>,
    /// Include untracked files (All and Unstaged, without Only selected files).
    pub untracked: bool,
}

impl SaveChangesForm {
    /// The dialog on `scope`, with only `path` ticked under Only selected files, or the whole
    /// scope.
    pub fn new(scope: WipScope, path: Option<String>) -> Self {
        Self { scope, pick: path.is_some(), chosen: path.into_iter().collect(), untracked: true }
    }

    /// The form a Save… message describes (see `PatchMsg::SaveChanges`).
    pub fn of(scope: WipScope, paths: Option<Vec<String>>, untracked: bool) -> Self {
        Self { scope, pick: paths.is_some(), chosen: paths.unwrap_or_default().into_iter().collect(), untracked }
    }

    /// The ticked paths that are still rows of the scope, in list order.
    pub fn ticked(&self, snap: &Snapshot) -> Vec<String> {
        rows(snap, self.scope).into_iter().filter(|f| self.chosen.contains(&f.path)).map(|f| f.path).collect()
    }
}

/// The files of `scope` the dialog lists: every working-tree file for All, else the staged or
/// unstaged ones. Conflicted rows and rows Trellis can't name safely are left out.
pub fn rows(snap: &Snapshot, scope: WipScope) -> Vec<FileEntry> {
    let all = match scope {
        WipScope::All => wip_files(snap),
        WipScope::Staged => snap.staged.clone(),
        WipScope::Unstaged => snap.unstaged.clone(),
    };
    all.into_iter().filter(|f| f.status != '!' && f.hazard.is_none()).collect()
}

/// Untracked files among `scope`'s rows (for "Include untracked files (N)").
pub fn untracked_count(snap: &Snapshot, scope: WipScope) -> usize {
    if scope == WipScope::Staged { 0 } else { rows(snap, scope).iter().filter(|f| f.status == '?').count() }
}

/// Why the form can't be saved now.
pub fn blocked(form: &SaveChangesForm, snap: &Snapshot) -> Option<&'static str> {
    if snap.unstaged.iter().any(|f| f.status == '!') {
        return Some(RESOLVE_FIRST);
    }
    if form.scope == WipScope::Staged && snap.staged.is_empty() {
        return Some(NOTHING_STAGED);
    }
    request(form, snap).err()
}

/// What Save… asks the core to write. With Only selected files off the tracked paths are left
/// empty, so git diffs every file of the scope.
pub fn request(form: &SaveChangesForm, snap: &Snapshot) -> Result<ChangesRequest, &'static str> {
    let rows = rows(snap, form.scope);
    let chosen: Vec<&FileEntry> = rows.iter().filter(|f| !form.pick || form.chosen.contains(&f.path)).collect();
    let with_untracked = form.scope != WipScope::Staged && (form.pick || form.untracked);
    let (untracked, tracked): (Vec<&FileEntry>, Vec<&FileEntry>) = chosen.into_iter().partition(|f| f.status == '?');
    let untracked: Vec<String> = if with_untracked { untracked.into_iter().map(|f| f.path.clone()).collect() } else { Vec::new() };
    if tracked.is_empty() && untracked.is_empty() {
        return Err(if form.pick { NO_PICK } else { NO_CHANGES });
    }
    Ok(ChangesRequest {
        scope: form.scope,
        paths: if form.pick { tracked.into_iter().map(|f| f.path.clone()).collect() } else { Vec::new() },
        picked: form.pick,
        untracked,
        unborn: snap.head_oid.is_none(),
    })
}

/// The paths a request names one by one (for the suggested file name): none unless picked.
pub fn named_paths(req: &ChangesRequest) -> Vec<String> {
    if req.picked { req.paths.iter().chain(&req.untracked).cloned().collect() } else { Vec::new() }
}

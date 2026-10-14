//! The index's submodules with their recorded and checked-out commits, classified for the left
//! panel.

use crate::error::Result;
use git2::{Oid, Repository, SubmoduleIgnore, SubmoduleStatus};
use std::path::Path;

/// How a submodule's checkout relates to the commit the parent records for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubmoduleState {
    /// Checked out at the recorded commit with nothing changed inside.
    InSync,
    /// The checked-out commit differs from the recorded one.
    OutOfSync,
    /// Uncommitted changes or untracked files inside, at the recorded commit.
    Dirty,
    /// Both of the above: another commit, and changes inside.
    DirtyOutOfSync,
    /// No repository in its folder (never cloned, deinitialized, or the folder was deleted).
    Uninitialized,
}

/// One submodule gitlink in the parent's index.
#[derive(Clone, PartialEq, Debug)]
pub struct SubmoduleInfo {
    /// The .gitmodules name, or the path for a gitlink without an entry there.
    pub name: String,
    /// Repo-relative, with '/'.
    pub path: String,
    /// As written in .gitmodules, so it may be relative; None when there is no entry.
    pub url: Option<String>,
    /// The gitlink in the parent's index.
    pub recorded: Oid,
    /// The submodule's HEAD; None when uninitialized.
    pub checked_out: Option<Oid>,
    pub state: SubmoduleState,
}

impl SubmoduleInfo {
    pub fn is_initialized(&self) -> bool {
        self.state != SubmoduleState::Uninitialized
    }

    pub fn is_out_of_sync(&self) -> bool {
        matches!(self.state, SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync)
    }

    pub fn is_dirty(&self) -> bool {
        matches!(self.state, SubmoduleState::Dirty | SubmoduleState::DirtyOutOfSync)
    }
}

/// The submodules of `repo` that have a gitlink in its index, sorted by path. Never fails: a
/// broken .gitmodules gives an empty list, and an entry whose status can't be read is
/// classified from its ids alone.
pub(super) fn list(repo: &Repository) -> Vec<SubmoduleInfo> {
    let Ok(subs) = repo.submodules() else { return Vec::new() };
    let mut out: Vec<SubmoduleInfo> = subs
        .iter()
        .filter_map(|sm| {
            let recorded = sm.index_id()?;
            let path = sm.path().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            let name = sm.name().map(str::to_owned).unwrap_or_else(|| path.clone());
            let checked_out = sm.workdir_id();
            let flags = repo.submodule_status(&name, SubmoduleIgnore::None).unwrap_or(SubmoduleStatus::empty());
            let state = classify(flags, recorded, checked_out);
            Some(SubmoduleInfo { name, path, url: sm.url().map(str::to_owned), recorded, checked_out, state })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The submodules of the repository at `workdir` (see `list`).
pub fn submodules_at(workdir: &Path) -> Result<Vec<SubmoduleInfo>> {
    let repo = Repository::open(workdir)?;
    Ok(list(&repo))
}

/// Compares ids rather than trusting WD_MODIFIED, so a failed status call still tells in sync
/// from out of sync.
fn classify(flags: SubmoduleStatus, recorded: Oid, checked_out: Option<Oid>) -> SubmoduleState {
    if checked_out.is_none() || flags.intersects(SubmoduleStatus::WD_UNINITIALIZED | SubmoduleStatus::WD_DELETED) {
        return SubmoduleState::Uninitialized;
    }
    let dirty = flags.intersects(SubmoduleStatus::WD_INDEX_MODIFIED | SubmoduleStatus::WD_WD_MODIFIED | SubmoduleStatus::WD_UNTRACKED);
    let out = checked_out != Some(recorded);
    match (dirty, out) {
        (false, false) => SubmoduleState::InSync,
        (false, true) => SubmoduleState::OutOfSync,
        (true, false) => SubmoduleState::Dirty,
        (true, true) => SubmoduleState::DirtyOutOfSync,
    }
}

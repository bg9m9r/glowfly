//! Working-tree file work (`Effect::Files`, runner/files.rs): restoring a file from a commit,
//! creating, renaming, deleting and discarding a working-tree file, the index and working-tree
//! edits of staging and conflicts (`LocalOp`), reverting part of a diff, and listing the tracked
//! files for the palette's file finder. Scoped to the repository's tab.

use super::EffectMeta;
use super::LocalOp;
use super::SandboxRule;
use crate::file_op::FileOp;
use git2::Oid;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum FilesEffect {
    /// Writes `path` from commit `oid` (or its parent) into the working tree only.
    /// Without `overwrite`, refuses when the file's current contents would be lost.
    Restore { workdir: PathBuf, oid: Oid, path: String, overwrite: bool },
    /// Creates, renames, deletes or discards (untracked) a working-tree file; the index is
    /// untouched, except for UnstageDelete, UnstageGone (index only) and a staged rename.
    Work { workdir: PathBuf, op: FileOp },
    /// The files in the index, for the palette's file finder.
    LoadTracked { workdir: PathBuf, generation: u64 },
    /// A quick index or working-tree edit: staging, discarding, a hunk or some lines, a conflict.
    LocalOp { workdir: PathBuf, op: LocalOp },
    /// Reverts part of a commit or comparison diff (a hunk, some of its lines, or the whole
    /// file) in the working tree only (glowfly_core::repo::revert).
    Revert { workdir: PathBuf, req: glowfly_core::repo::RevertRequest },
}

impl EffectMeta for FilesEffect {
    fn name(&self) -> &'static str {
        match self {
            FilesEffect::Restore { .. } => "restore-file",
            FilesEffect::Work { .. } => "work-file",
            FilesEffect::LoadTracked { .. } => "tracked-paths",
            FilesEffect::LocalOp { .. } => "local-op",
            FilesEffect::Revert { .. } => "revert",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            FilesEffect::Restore { workdir, .. }
            | FilesEffect::Work { workdir, .. }
            | FilesEffect::LoadTracked { workdir, .. }
            | FilesEffect::LocalOp { workdir, .. }
            | FilesEffect::Revert { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// A restore, a file change, an index edit and a revert wait for the repository's other
    /// changes; the listing only reads.
    fn queue(&self) -> Option<&Path> {
        match self {
            FilesEffect::Restore { workdir, .. }
            | FilesEffect::Work { workdir, .. }
            | FilesEffect::LocalOp { workdir, .. }
            | FilesEffect::Revert { workdir, .. } => Some(workdir),
            FilesEffect::LoadTracked { .. } => None,
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

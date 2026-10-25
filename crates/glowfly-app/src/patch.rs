//! Messages and effects of saving patch files (commits, one file of a commit, uncommitted
//! changes), copying a commit's patch, and applying a patch file (its changes, or as commits)
//! (state/patch*.rs, update/patch*.rs, runner/patches.rs, the graph and file menus, the Create
//! patch and Apply patch dialogs).

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use git2::Oid;
use glowfly_core::patch_ops::{ApplyOptions, ExportReport, PatchSource, PatchSummary, SaveReport, WipScope};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum PatchMsg {
    /// From the graph menus: save these commits (in any order) as one patch file.
    Export(Vec<Oid>),
    /// From a commit's file row: save commit `oid` limited to `paths` as a patch file.
    ExportFiles { oid: Oid, paths: Vec<String> },
    /// Open the Create patch dialog on uncommitted changes of `scope`, with only `path` ticked
    /// under Only selected files (or the whole scope).
    ChangesDialog { scope: WipScope, path: Option<String> },
    /// From the Create patch dialog: save the uncommitted changes of `scope`; `paths` None means
    /// every file of the scope (with untracked files when `untracked`), Some the ticked list.
    SaveChanges { scope: WipScope, paths: Option<Vec<String>>, untracked: bool },
    /// From the save dialog: where to save `what` of `workdir`; None when it was cancelled.
    SaveTo { workdir: PathBuf, what: PatchSource, dest: Option<PathBuf> },
    /// The save into `dest` ended.
    Exported { dest: PathBuf, result: Result<SaveReport, String> },
    /// From a saved patch's toast: show it in the file manager.
    ShowSaved(PathBuf),
    /// Put the patch of these commits (in any order) on the clipboard.
    Copy(Vec<Oid>),
    /// The patch text to copy, with what it holds.
    Copied(Result<(String, ExportReport), String>),
    /// From the palette: pick a patch file to apply.
    PickToApply,
    /// From the open dialog: read `path` for the Apply patch dialog of `workdir`; None when it
    /// was cancelled.
    ApplyFrom { workdir: PathBuf, path: Option<PathBuf> },
    /// The patch file at `path` was read for `workdir`'s Apply patch dialog.
    Inspected { workdir: PathBuf, path: PathBuf, result: Result<PatchSummary, String> },
    /// From the Apply patch dialog.
    Apply(ApplyRequest),
}

impl PatchMsg {
    /// Never scoped: the effects behind these messages are unscoped and carry their repository,
    /// so nothing is dropped when tabs change.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Export(..)
            | Self::ExportFiles { .. }
            | Self::ChangesDialog { .. }
            | Self::SaveChanges { .. }
            | Self::SaveTo { .. }
            | Self::Exported { .. }
            | Self::ShowSaved(_)
            | Self::Copy(_)
            | Self::Copied(_)
            | Self::PickToApply
            | Self::ApplyFrom { .. }
            | Self::Inspected { .. }
            | Self::Apply(_) => None,
        }
    }
}

/// Applying the patch file at `path` to `workdir`, as the Apply patch dialog chose.
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyRequest {
    pub workdir: PathBuf,
    pub path: PathBuf,
    pub how: ApplyHow,
    /// Emailed commits in the file.
    pub commits: usize,
}

/// How a patch is applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApplyHow {
    /// Its changes, with `git apply`.
    Changes(ApplyOptions),
    /// As commits, with `git am`.
    Commits { three_way: bool },
}

/// The patch dialogs, saving, copying and reading a patch file, run by runner/patches.rs.
#[derive(Debug)]
pub enum PatchEffect {
    /// The save dialog for a patch of `what` of `workdir`, suggesting `name`.
    PickSave { workdir: PathBuf, what: PatchSource, name: String },
    /// Writes the patch of `what` of `workdir` into `dest`.
    Export { workdir: PathBuf, what: PatchSource, dest: PathBuf },
    /// The open dialog for a patch to apply to `workdir`.
    PickOpen { workdir: PathBuf },
    /// Reads the patch file at `path` (no git) for `workdir`'s Apply patch dialog.
    Inspect { workdir: PathBuf, path: PathBuf },
    /// The patch text of `oids` (oldest first) of `workdir`, for the clipboard.
    Copy { workdir: PathBuf, oids: Vec<Oid> },
}

impl EffectMeta for PatchEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            PatchEffect::PickSave { .. } => "pick-patch-save",
            PatchEffect::Export { .. } => "export-patch",
            PatchEffect::PickOpen { .. } => "pick-patch",
            PatchEffect::Inspect { .. } => "inspect-patch",
            PatchEffect::Copy { .. } => "copy-patch",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Whether the runner times this for the performance log: the file dialogs wait on the user.
    fn timed(&self) -> bool {
        match self {
            PatchEffect::Export { .. } | PatchEffect::Inspect { .. } | PatchEffect::Copy { .. } => true,
            PatchEffect::PickSave { .. } | PatchEffect::PickOpen { .. } => false,
        }
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    /// The file dialogs are dropped.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        match self {
            PatchEffect::Export { .. } | PatchEffect::Inspect { .. } | PatchEffect::Copy { .. } => SandboxRule::Run,
            PatchEffect::PickSave { .. } | PatchEffect::PickOpen { .. } => SandboxRule::Drop,
        }
    }
}

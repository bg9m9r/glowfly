//! Messages and effects of saving commits as a patch file and applying one to the working tree
//! (state/patch.rs, update/patch.rs, runner/patches.rs, the graph menus in ui/menus.rs).

use git2::Oid;
use std::path::PathBuf;
use trellis_core::patch_ops::ExportReport;

#[derive(Clone, Debug, PartialEq)]
pub enum PatchMsg {
    /// From the graph menus: save these commits (in any order) as one patch file.
    Export(Vec<Oid>),
    /// From the save dialog: where to save `oids` (oldest first) of `workdir`; None when it was
    /// cancelled.
    SaveTo { workdir: PathBuf, oids: Vec<Oid>, dest: Option<PathBuf> },
    /// The export into `dest` ended.
    Exported { dest: PathBuf, result: Result<ExportReport, String> },
    /// From the palette: pick a patch file to apply.
    PickToApply,
    /// From the open dialog: apply `path` to `workdir`'s working tree; None when it was
    /// cancelled.
    ApplyFrom { workdir: PathBuf, path: Option<PathBuf> },
}

impl PatchMsg {
    /// Never scoped: the effects behind these messages are unscoped and carry their repository,
    /// so nothing is dropped when tabs change.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Export(..) | Self::SaveTo { .. } | Self::Exported { .. } | Self::PickToApply | Self::ApplyFrom { .. } => None,
        }
    }
}

/// The patch dialogs and the export, run by runner/patches.rs.
#[derive(Debug)]
pub enum PatchEffect {
    /// The save dialog for a patch of `oids` (oldest first) of `workdir`, suggesting `name`.
    PickSave { workdir: PathBuf, oids: Vec<Oid>, name: String },
    /// Writes `oids` (oldest first) of `workdir` into `dest` as one patch file.
    Export { workdir: PathBuf, oids: Vec<Oid>, dest: PathBuf },
    /// The open dialog for a patch to apply to `workdir`.
    PickOpen { workdir: PathBuf },
}

impl PatchEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            PatchEffect::PickSave { .. } => "pick-patch-save",
            PatchEffect::Export { .. } => "export-patch",
            PatchEffect::PickOpen { .. } => "pick-patch",
        }
    }

    /// Whether the runner times this for the performance log: the file dialogs wait on the user.
    pub fn timed(&self) -> bool {
        match self {
            PatchEffect::Export { .. } => true,
            PatchEffect::PickSave { .. } | PatchEffect::PickOpen { .. } => false,
        }
    }
}

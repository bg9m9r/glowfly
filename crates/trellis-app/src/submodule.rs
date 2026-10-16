//! Messages of the SUBMODULES section and the Add submodule and Remove submodule dialogs
//! (state/submodules.rs, state/submodules/add.rs, state/submodules/remove.rs,
//! update/submodules.rs, update/submodule_add.rs, update/submodule_remove.rs, ui/submodules.rs,
//! ui/submodule_dialogs.rs).

use std::path::PathBuf;
use trellis_core::repo::SubmoduleInfo;
use trellis_core::submodule_ops::{RemovalFacts, RemoveReport};

/// A remove request, checked by update; the runner deinitializes the submodule, runs `git rm`,
/// then deletes its stored module.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveRequest {
    /// The workdir of the tab the request came from.
    pub repo: PathBuf,
    /// Repo-relative, with '/'.
    pub path: String,
    /// The .gitmodules name, which names the stored module.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SubmoduleMsg {
    /// Open the initialized submodule at `path` (repo-relative) as a repository in a tab.
    Open(String),
    /// `git submodule update --init --checkout -- <path>` as a job; out of sync asks first (`confirmed: false`).
    Update { path: String, confirmed: bool },
    /// The submodule at `path` read again before asking to update it (`SubmoduleEffect::Check`);
    /// None when it is gone.
    Checked { path: String, result: Result<Option<SubmoduleInfo>, String> },
    /// Open the Add submodule dialog.
    AddDialog,
    /// The Add dialog's URL or path text changed: check the folder on disk again.
    AddEdited,
    /// From the runner: whether git can add a submodule at `path` in `repo` (Ok holds warnings).
    AddPathChecked { repo: PathBuf, path: String, result: Result<Vec<String>, String> },
    /// The Add dialog's button: reads the form from `st.modal`.
    Add,
    /// Remove the submodule at this path: read it again, then ask.
    RemoveDialog(String),
    /// The submodule read again before asking to remove it; None when it is gone.
    RemoveChecked { path: String, result: Result<Option<Box<RemovalFacts>>, String> },
    /// The Remove dialog's button: reads the form from `st.modal`.
    Remove,
    /// From the runner: how the removal went. It ends the active job.
    Removed { request: RemoveRequest, report: RemoveReport },
}

impl SubmoduleMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. Only `Checked` and `RemoveChecked` come back scoped, and both only lead to a
    /// question for the tab that asked.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(..)
            | Self::Update { .. }
            | Self::Checked { .. }
            | Self::AddDialog
            | Self::AddEdited
            | Self::AddPathChecked { .. }
            | Self::Add
            | Self::RemoveDialog(..)
            | Self::RemoveChecked { .. }
            | Self::Remove
            | Self::Removed { .. } => None,
        }
    }
}

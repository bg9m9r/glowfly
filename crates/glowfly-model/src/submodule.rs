//! Messages of the SUBMODULES section, its batch updates, the Add, Edit and Remove submodule
//! dialogs, and the work done inside a submodule from its row (state/submodules.rs,
//! state/submodules/{add,batch,edit,inside,remove}.rs, update/submodules.rs,
//! update/submodule_{add,batch,edit,inside,remove}.rs, ui/submodules.rs, ui/submodule_dialogs.rs).

use glowfly_core::repo::{InsideHead, SubmoduleInfo};
use glowfly_core::submodule_ops::{RemovalFacts, RemoveReport};
use std::path::PathBuf;

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

/// Which batch of submodules to update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Batch {
    /// Initialize every one not checked out (the header menu, the palette, the offer on open).
    InitAll,
    /// Initialize or update every one not at its recorded commit (the header menu, the palette).
    UpdateAll,
    /// Like `UpdateAll`, after a checkout, pull, merge, rebase or reset, leaving out any that
    /// would leave commits behind.
    Auto,
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
    /// Start a batch update. Unconfirmed reads the submodules again first (and Update all may
    /// ask); confirmed runs from the snapshot.
    Batch { batch: Batch, confirmed: bool },
    /// From the runner: the submodules read again for `batch`.
    CheckedAll { batch: Batch, result: Result<Vec<SubmoduleInfo>, String> },
    /// The header menu's choice about updating after checkouts and pulls in this repository.
    SetUpdates(glowfly_core::settings::SubmoduleUpdates),
    /// Open the Edit submodule dialog for the submodule at this path.
    EditDialog(String),
    /// The Edit dialog's Save: reads the form from `st.modal`.
    Edit,
    /// Record this checkout: stage the submodule's checked-out commit as its gitlink.
    Record(String),
    /// Fetch inside the submodule at this path, as a job in its folder.
    Fetch(String),
    /// Pull inside the submodule at this path: its HEAD is read first (`HeadRead`).
    Pull(String),
    /// From the runner: the submodule's HEAD read for Pull; None when it isn't checked out.
    HeadRead { path: String, result: Result<Option<InsideHead>, String> },
    /// Discard changes inside the submodule at `path` (a stash there); unconfirmed asks first.
    DiscardInside { path: String, confirmed: bool },
}

impl SubmoduleMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. Only `Checked`, `RemoveChecked`, `CheckedAll` and `HeadRead` come back scoped,
    /// and each only leads to a question or a job for the tab that asked.
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
            | Self::Removed { .. }
            | Self::Batch { .. }
            | Self::CheckedAll { .. }
            | Self::SetUpdates(..)
            | Self::EditDialog(..)
            | Self::Edit
            | Self::Record(..)
            | Self::Fetch(..)
            | Self::Pull(..)
            | Self::HeadRead { .. }
            | Self::DiscardInside { .. } => None,
        }
    }
}

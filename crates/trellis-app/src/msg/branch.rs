//! Messages of the branch menus and the Clone dialog (update/branches.rs, update/clone.rs).

/// The New branch, rename and upstream dialogs and the branch menus' actions (update/branches.rs).
pub enum BranchMsg {
    /// Open the rename dialog for local branch `.0`.
    RenameBranchDialog(String),
    /// `git branch -m old new`.
    RenameBranch {
        old: String,
        new: String,
    },
    /// Open the upstream dialog for local branch `.0`.
    SetUpstreamDialog(String),
    /// Make `branch` track `name` on `remote` (which need not exist yet).
    SetUpstream {
        branch: String,
        remote: String,
        name: String,
    },
    /// Remove the upstream setting of local branch `.0`.
    UnsetUpstream(String),
    /// Fast-forward local branch `.0` to its upstream, without checking it out.
    FastForwardBranch(String),
    /// Delete local branch `.0` and then its upstream branch on the remote.
    DeleteBranchAndRemote(String),
    /// Push local `branch` to `remote`; None means its upstream, or the default remote with -u.
    PushBranch {
        branch: String,
        remote: Option<String>,
    },
    NewBranch {
        start: Option<String>,
        name: String,
    },
}

impl BranchMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::RenameBranchDialog(..)
            | Self::RenameBranch { .. }
            | Self::SetUpstreamDialog(..)
            | Self::SetUpstream { .. }
            | Self::UnsetUpstream(..)
            | Self::FastForwardBranch(..)
            | Self::DeleteBranchAndRemote(..)
            | Self::PushBranch { .. }
            | Self::NewBranch { .. } => None,
        }
    }
}

/// The Clone dialog and the clone it starts (update/clone.rs).
pub enum CloneMsg {
    /// Open the Clone dialog.
    Dialog,
    /// The Clone dialog's parent folder text changed: check it on disk after a short pause.
    ParentEdited,
    /// Whether the Clone dialog's parent folder (the trimmed text `parent`) is a folder.
    ParentChecked { parent: String, is_dir: bool },
    /// Clone a repository into a folder (the Clone dialog's request, options included).
    Start(crate::state::clone::CloneRequest),
}

impl CloneMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Dialog | Self::ParentEdited | Self::ParentChecked { .. } | Self::Start(..) => None,
        }
    }
}

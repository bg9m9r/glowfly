//! Messages of the branch menus and the Clone dialog (update/branches.rs, update/clone.rs).

/// The New branch, rename and upstream dialogs and the branch menus' actions (update/branches.rs).
pub enum BranchMsg {
    /// Check out local branch `.0`, unless it is HEAD or checked out in another worktree.
    Checkout(String),
    /// Check out the local branch tracking remote branch `name` on `remote`, or create one
    /// tracking it.
    CheckoutRemote {
        remote: String,
        name: String,
    },
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
            Self::Checkout(..)
            | Self::CheckoutRemote { .. }
            | Self::RenameBranchDialog(..)
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
    /// What the Clone dialog's parent folder text (trimmed, `parent`) names: the folder, `~`
    /// expanded and resolved, or None when it isn't one.
    ParentChecked { parent: String, found: Option<std::path::PathBuf> },
    /// Clone a repository into a folder (the Clone dialog's request, options included).
    Start(crate::state::clone::CloneRequest),
    /// Switch the Clone dialog between the URL tab and the Your repositories tab.
    Source(crate::state::clone_service::CloneSource),
    /// Choose account `.0` (an index into the tab's accounts).
    ServiceAccount(usize),
    /// Ask for the chosen account's repositories again after a failure.
    RetryRepos,
    /// Pick repository `.0` (an index into the unfiltered list) and fill in its clone URL.
    PickRepo(usize),
    /// Fill in the SSH clone URL (true) or the https one.
    UseSsh(bool),
    /// The repositories of `account`, or why they couldn't be listed.
    ReposLoaded { account: trellis_core::forge::Account, result: Box<Result<trellis_core::forge::MyRepos, crate::effect::ForgeError>> },
    /// A sparse clone job ended (`CloneEffect::Sparse`).
    Finished(trellis_core::clone_ops::CloneOutcome),
}

impl CloneMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. The Clone dialog's effects are unscoped, so their results always reach
    /// `update` and are never wrapped in `Msg::Scoped`.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Dialog
            | Self::ParentEdited
            | Self::ParentChecked { .. }
            | Self::Start(..)
            | Self::Source(..)
            | Self::ServiceAccount(..)
            | Self::RetryRepos
            | Self::PickRepo(..)
            | Self::UseSsh(..)
            | Self::ReposLoaded { .. }
            | Self::Finished(..) => None,
        }
    }
}

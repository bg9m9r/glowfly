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
    /// Check out the source branch of the listed pull request `id` (update/pr_checkout.rs):
    /// its local branch, or a new one tracking the code host's remote.
    CheckoutPullRequest {
        id: u64,
    },
    /// Open the rename dialog for local branch `.0`.
    RenameBranchDialog(String),
    /// `git branch -m old new`; with `remote`, then the new name pushed to the upstream's remote
    /// (and made the upstream) and the old one deleted there (update/branch_sync.rs).
    RenameBranch {
        old: String,
        new: String,
        remote: bool,
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
    /// Open the Delete dialog for local branch `.0` and count the commits only it has
    /// (update/branch_delete.rs).
    DeleteDialog(String),
    /// How many commits only local branch `name` has at `tip`, or why they couldn't be counted
    /// (`BranchEffect::CountUnmerged`).
    UnmergedCounted {
        name: String,
        tip: git2::Oid,
        count: Result<usize, String>,
    },
    /// The Delete dialog's danger button: delete local branch `name` (`git branch -d`, or `-D`
    /// when `force`), unless it has moved off `tip`.
    Delete {
        name: String,
        tip: git2::Oid,
        force: bool,
    },
    /// Open the Delete remote branch dialog for `name` on `remote` (update/branch_sync.rs).
    DeleteRemoteDialog {
        remote: String,
        name: String,
    },
    /// Delete branch `name` on `remote`, then unset the upstream of the local branches `unset`.
    DeleteRemote {
        remote: String,
        name: String,
        unset: Vec<String>,
    },
    /// Delete local branch `.0` and then its upstream branch on the remote.
    DeleteBranchAndRemote(String),
    /// Push local `branch` to `remote`; None means its upstream, or the default remote with -u.
    PushBranch {
        branch: String,
        remote: Option<String>,
    },
    /// Force push local `branch` to its upstream with lease, expecting the upstream at `seen`
    /// (None: not on the remote), as its confirmation said (update/branch_push.rs).
    ForcePush {
        branch: String,
        seen: Option<git2::Oid>,
    },
    /// Push local branch `.0` to every remote, one after another (update/branch_push.rs).
    PushAll(String),
    /// Push to all remotes ended (`BranchEffect::PushAll`); unscoped like RunGit.
    PushedAll {
        branch: String,
        total: usize,
        result: glowfly_core::push_all::PushedAll,
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
            | Self::CheckoutPullRequest { .. }
            | Self::RenameBranchDialog(..)
            | Self::RenameBranch { .. }
            | Self::SetUpstreamDialog(..)
            | Self::SetUpstream { .. }
            | Self::UnsetUpstream(..)
            | Self::FastForwardBranch(..)
            | Self::DeleteDialog(..)
            | Self::UnmergedCounted { .. }
            | Self::Delete { .. }
            | Self::DeleteRemoteDialog { .. }
            | Self::DeleteRemote { .. }
            | Self::DeleteBranchAndRemote(..)
            | Self::PushBranch { .. }
            | Self::ForcePush { .. }
            | Self::PushAll(..)
            | Self::PushedAll { .. }
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
    ReposLoaded { account: glowfly_core::forge::Account, result: Box<Result<glowfly_core::forge::MyRepos, crate::effect::ForgeError>> },
    /// A sparse clone job ended (`CloneEffect::Sparse`).
    Finished(glowfly_core::clone_ops::CloneOutcome),
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

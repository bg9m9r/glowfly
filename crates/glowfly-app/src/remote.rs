//! Messages and effects of the REMOTE section: the Add/Edit remote dialog (state/remotes.rs,
//! update/remotes.rs, ui/remotes/), viewing a remote on its hosting service, and the fork
//! dialogs (state/forks.rs, update/remote_forks.rs, runner/forks.rs).

use crate::effect::EffectMeta;
use crate::effect::ForgeError;
use crate::effect::SandboxRule;
use glowfly_core::forge::{Auth, CreatedFork, ForkList, ForkLookup, Target};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteMsg {
    /// Open the Add remote dialog.
    AddDialog,
    /// Open the Edit remote dialog for this remote.
    EditDialog(String),
    /// The dialog's Add remote / Save: reads the form from `st.modal`.
    Submit,
    /// `git fetch --prune <remote>`.
    Fetch(String),
    /// Ask before removing this remote (opens Modal::Confirm).
    RemoveDialog(String),
    /// Confirmed: `git remote remove <remote>`.
    Remove(String),
    /// Open <remote>'s page on its hosting service, or log why there is none.
    View(String),
    /// Open the Add a fork dialog for <remote>'s repository and list its forks.
    ForkDialog(String),
    /// Pick the fork at this index of the dialog's (unfiltered) list.
    PickFork(usize),
    /// The Add a fork dialog's Add remote.
    AddFork,
    /// Open the Fork dialog for <remote> and look for the user's own fork.
    ForkOnService(String),
    /// The Fork dialog's Add as remote (the fork the user already has).
    AddMyFork,
    /// The Fork dialog's Fork and add remote.
    CreateFork,
    /// Retry the open fork dialog's failed lookup.
    RetryForks,
    // From the runner (unscoped; matched against the open dialog's workdir and target). The
    // results are boxed to keep messages small.
    ForksLoaded {
        workdir: PathBuf,
        target: Target,
        result: Box<Result<ForkList, ForgeError>>,
    },
    MyForkFound {
        workdir: PathBuf,
        target: Target,
        result: Box<Result<ForkLookup, ForgeError>>,
    },
    ForkCreated {
        workdir: PathBuf,
        target: Target,
        name: String,
        result: Box<Result<CreatedFork, ForgeError>>,
    },
}

impl RemoteMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. The fork effects are unscoped, so their results never arrive scoped.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::AddDialog | Self::EditDialog(..) | Self::Submit | Self::Fetch(..) | Self::RemoveDialog(..) | Self::Remove(..) => None,
            Self::View(..) | Self::ForkDialog(..) | Self::PickFork(..) | Self::AddFork | Self::ForkOnService(..) => None,
            Self::AddMyFork | Self::CreateFork | Self::RetryForks => None,
            Self::ForksLoaded { .. } | Self::MyForkFound { .. } | Self::ForkCreated { .. } => None,
        }
    }
}

/// A hosting-service request for the fork dialogs; `Debug` hides the credential (Auth's own
/// `Debug`).
#[derive(Debug)]
pub struct ForkRequest {
    pub workdir: PathBuf,
    pub target: Target,
    /// A credential from this session; None signs in (environment, saved token, Git Credential
    /// Manager).
    pub auth: Option<Auth>,
    /// The user connected this account in Preferences.
    pub connected: bool,
}

#[derive(Debug)]
pub enum RemoteEffect {
    ListForks(ForkRequest),
    FindMyFork(ForkRequest),
    CreateFork { req: ForkRequest, name: String },
}

impl EffectMeta for RemoteEffect {
    fn name(&self) -> &'static str {
        match self {
            RemoteEffect::ListForks(_) => "list-forks",
            RemoteEffect::FindMyFork(_) => "find-my-fork",
            RemoteEffect::CreateFork { .. } => "create-fork",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}

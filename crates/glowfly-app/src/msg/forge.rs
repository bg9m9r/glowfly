//! Messages of hosting services, publishing and credential prompts (update/forge.rs,
//! update/publish.rs, update/askpass.rs).

use crate::effect::ForgeReply;
use glowfly_core::forge::{Account, CreatedRepo, NewPullRequest, TrelloBoard};
use glowfly_core::settings::TrackerChoice;
use std::path::PathBuf;

/// Credential prompts from git and ssh through the askpass helper (state/askpass.rs,
/// update/askpass.rs).
pub enum CredentialMsg {
    /// git or ssh asks for input through the askpass helper (from the listener; unscoped).
    CredentialPrompt { id: u64, prompt: glowfly_core::askpass::Prompt },
    /// Prompt `id` was not answered in time and has been closed.
    CredentialPromptExpired(u64),
    /// Sign in / OK / Unlock / Yes: answer the shown prompt from the dialog's text buffers.
    AnswerCredentials,
    /// Cancel, No or Esc.
    CancelCredentials,
    /// Saving an HTTPS sign-in for `host` with git's credential helper ended (runner/credentials.rs).
    Remembered { host: String, result: Result<glowfly_core::git_credential::Remembered, String> },
    /// Dropping a rejected HTTPS sign-in for `host` from git's credential helper ended.
    Forgotten { host: String, result: Result<(), String> },
    /// The sign-in banner's "Fetch <remote>": fetch it in a job that can prompt (update/sign_in.rs).
    SignInFetch { workdir: PathBuf, remote: String },
    /// The sign-in banner's "Not now": hide the remote's line; auto-fetch still skips it.
    DismissSignIn { workdir: PathBuf, remote: String },
}

impl CredentialMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::CredentialPrompt { .. }
            | Self::CredentialPromptExpired(..)
            | Self::AnswerCredentials
            | Self::CancelCredentials
            | Self::Remembered { .. }
            | Self::Forgotten { .. }
            | Self::SignInFetch { .. }
            | Self::DismissSignIn { .. } => None,
        }
    }
}

/// Hosting services: refreshing pull requests and issues, creating a pull request, and
/// connecting accounts (update/forge.rs).
pub enum ForgeMsg {
    ForgeRefresh {
        interactive: bool,
    },
    /// Verify and save a connection; `token: None` signs in through Git Credential Manager.
    Connect {
        account: Account,
        user: Option<String>,
        token: Option<String>,
    },
    Disconnect(Account),
    /// Disconnect everything; `gcm` also signs Git Credential Manager out of those hosts.
    ForgetTokens {
        gcm: bool,
    },
    SetTracker(TrackerChoice),
    /// Open the create dialog for `source` (a local or host-remote branch), preselecting `target`.
    CreatePrDialog {
        source: String,
        target: Option<String>,
    },
    /// Push local `branch` to the code host's remote, then open the create dialog.
    PushAndStartPr {
        branch: String,
        target: Option<String>,
    },
    CreatePr(NewPullRequest),
    ForgeReplied(ForgeReply),
    /// The branch tips drawn this frame (LOCAL and REMOTE rows, graph ref labels), sent by the
    /// shell when the set changes.
    TipsShown(Vec<git2::Oid>),
    Connected {
        account: Account,
        method: String,
        token: Option<(Option<String>, String)>,
        result: Result<String, String>,
    },
    /// Repo-specific preferences > Issue tracker from the side list; loads Trello boards when
    /// Trello is chosen.
    ShowTrackerPage,
    /// Load (or reload) a connected Trello account's boards for the Board picker.
    LoadTrelloBoards(Account),
    TrelloBoardsLoaded {
        account: Account,
        result: Result<Vec<TrelloBoard>, String>,
    },
}

impl ForgeMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::ForgeRefresh { .. }
            | Self::Connect { .. }
            | Self::Disconnect(..)
            | Self::ForgetTokens { .. }
            | Self::SetTracker(..)
            | Self::CreatePrDialog { .. }
            | Self::PushAndStartPr { .. }
            | Self::CreatePr(..)
            | Self::ForgeReplied(..)
            | Self::TipsShown(..)
            | Self::Connected { .. }
            | Self::ShowTrackerPage
            | Self::LoadTrelloBoards(..)
            | Self::TrelloBoardsLoaded { .. } => None,
        }
    }
}

/// Creating a hosted repository for a repository with no remote, then pushing to it
/// (update/publish.rs).
pub enum PublishMsg {
    /// Offer to create a GitHub repository for a repository with no remote, then push to it.
    PublishDialog,
    PublishAccount(usize),
    PublishOwners {
        account: Account,
        result: Result<Vec<String>, String>,
    },
    Publish,
    Published {
        workdir: PathBuf,
        branch: String,
        result: Result<CreatedRepo, String>,
    },
}

impl PublishMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::PublishDialog | Self::PublishAccount(..) | Self::PublishOwners { .. } | Self::Publish | Self::Published { .. } => None,
        }
    }
}

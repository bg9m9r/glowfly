//! Messages of hosting services, publishing and credential prompts (update/forge.rs,
//! update/publish.rs, update/askpass.rs).

use crate::effect::ForgeReply;
use std::path::PathBuf;
use trellis_core::forge::{Account, CreatedRepo, NewPullRequest};
use trellis_core::settings::TrackerChoice;

/// Credential prompts from git and ssh through the askpass helper (state/askpass.rs,
/// update/askpass.rs).
pub enum CredentialMsg {
    /// git or ssh asks for input through the askpass helper (from the listener; unscoped).
    CredentialPrompt { id: u64, prompt: trellis_core::askpass::Prompt },
    /// Prompt `id` was not answered in time and has been closed.
    CredentialPromptExpired(u64),
    /// Sign in / OK / Unlock / Yes: answer the shown prompt from the dialog's text buffers.
    AnswerCredentials,
    /// Cancel, No or Esc.
    CancelCredentials,
}

impl CredentialMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::CredentialPrompt { .. } | Self::CredentialPromptExpired(..) | Self::AnswerCredentials | Self::CancelCredentials => None,
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
    CreatePrDialog(String),
    CreatePr(NewPullRequest),
    ForgeReplied(ForgeReply),
    Connected {
        account: Account,
        method: String,
        token: Option<(Option<String>, String)>,
        result: Result<String, String>,
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
            | Self::CreatePrDialog(..)
            | Self::CreatePr(..)
            | Self::ForgeReplied(..)
            | Self::Connected { .. } => None,
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

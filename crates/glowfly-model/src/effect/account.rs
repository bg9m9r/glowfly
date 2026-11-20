//! Connected accounts on hosting services (`Effect::Account`): signing in, saving and dropping
//! tokens, creating a repository and the lists Preferences and the Publish dialog read.
//! Unscoped: they run outside any repository's tab.

use super::EffectMeta;
use super::SandboxRule;
use super::partial::StopKey;
use glowfly_core::forge::{Account, NewRepo};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug)]
pub enum AccountEffect {
    /// Where the signed-in user can create repositories (their login, then organizations).
    Owners {
        account: Account,
        cwd: PathBuf,
    },
    /// Creates a repository for `workdir`; the result carries what is needed to push to it.
    /// `cwd` is only where credentials are looked up.
    CreateRepo {
        account: Account,
        cwd: PathBuf,
        workdir: PathBuf,
        user: String,
        repo: NewRepo,
        branch: String,
    },
    Connect {
        account: Account,
        user: Option<String>,
        token: Option<String>,
        cwd: PathBuf,
    },
    SaveToken {
        account: Account,
        user: Option<String>,
        token: String,
    },
    DeleteToken(Account),
    ForgetCredentialManager {
        accounts: Vec<Account>,
        cwd: PathBuf,
    },
    /// The open boards of a connected Trello account (Issue Tracker > Board).
    TrelloBoards {
        account: Account,
        cwd: PathBuf,
    },
    /// Sign in with Trello: opens Trello's "Allow Glowfly" page for `key` in the browser and
    /// waits for the token on a local port (`ForgeMsg::TrelloSignInEnded`); stopped by
    /// `StopGit(StopKey::TrelloSignIn)`. `started` tells this wait from a later one.
    TrelloSignIn {
        key: String,
        started: Instant,
    },
}

impl EffectMeta for AccountEffect {
    fn name(&self) -> &'static str {
        match self {
            AccountEffect::Owners { .. } => "repo-owners",
            AccountEffect::CreateRepo { .. } => "create-repo",
            AccountEffect::Connect { .. } => "connect",
            AccountEffect::SaveToken { .. } => "save-token",
            AccountEffect::DeleteToken(_) => "delete-token",
            AccountEffect::ForgetCredentialManager { .. } => "forget-gcm",
            AccountEffect::TrelloBoards { .. } => "trello-boards",
            AccountEffect::TrelloSignIn { .. } => "trello-sign-in",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Not Sign in with Trello: it waits on the user.
    fn timed(&self) -> bool {
        !matches!(self, AccountEffect::TrelloSignIn { .. })
    }

    fn stop_key(&self) -> Option<StopKey> {
        matches!(self, AccountEffect::TrelloSignIn { .. }).then_some(StopKey::TrelloSignIn)
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}

//! Messages and effects of Preferences > SSH: the key pairs in ~/.ssh, generating a key, the
//! key git uses (core.sshCommand), the local SSH agent and adding a key to a hosting service
//! (state/ssh*.rs, update/ssh*.rs, runner/ssh.rs, ui/prefs/ssh*.rs).

use crate::msg::Inactive;
use std::path::PathBuf;
use trellis_core::forge::{Account, AddKeyOutcome};
use trellis_core::ssh_keys::{AgentReach, AgentStatus, KeyChoice, NewKey, SshKeyPair, SshListing};

pub enum SshMsg {
    /// The navigation row: switch Preferences to the page and read ~/.ssh.
    ShowPage,
    /// Read ~/.ssh again (the Refresh button).
    Reload,
    /// Copy the public key of the listed pair whose private key is at this path.
    CopyPublicKey(PathBuf),
    /// The Generate Key button, or accepting the Replace confirmation (`overwrite`).
    Generate {
        overwrite: bool,
    },
    /// Use for git: make git use this key for SSH remotes, for every repository or (`repo`)
    /// the open one; `confirmed` once the user agreed to replace a command Trellis didn't write.
    UseKey {
        key: PathBuf,
        repo: bool,
        confirmed: bool,
    },
    /// Stop using: unset core.sshCommand at one level.
    StopUsing {
        repo: bool,
        confirmed: bool,
    },
    /// The Use local SSH agent checkbox.
    SetUseAgent(bool),
    /// Check the agent again (its Refresh button).
    ReloadAgent,
    /// Add the listed key with this private key to the agent.
    AddToAgent(PathBuf),
    /// Add to service…: one connected account picked for a key.
    ChooseService {
        key: PathBuf,
        account: Account,
    },
    /// The inline form's Add key button.
    SendToService,
    CancelService,
    /// Copy the key and open the account's SSH key settings page.
    OpenKeySettings {
        key: PathBuf,
        account: Account,
    },
    // ---- from the runner
    Loaded {
        generation: u64,
        result: Result<SshListing, String>,
    },
    Generated(Result<SshKeyPair, GenerateFailure>),
    ChoiceLoaded {
        generation: u64,
        global: Result<KeyChoice, String>,
        /// None when no repository was read.
        repo: Option<Result<KeyChoice, String>>,
    },
    KeyChoiceSet {
        repo: bool,
        result: Result<(), String>,
    },
    AgentLoaded {
        generation: u64,
        status: AgentStatus,
    },
    AgentAdded {
        key: PathBuf,
        result: Result<(), String>,
    },
    AddedToService {
        key: PathBuf,
        account: Account,
        result: Result<AddKeyOutcome, ServiceKeyError>,
    },
}

/// Why a key wasn't generated.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerateFailure {
    /// A key (or its .pub) is already at this path: ask before replacing it.
    Exists(PathBuf),
    Failed(String),
}

/// Why a key wasn't added to a service; `fallback` when copying the key and opening the
/// service's settings page is the way forward (no permission, or no API).
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceKeyError {
    pub message: String,
    pub fallback: bool,
}

impl SshMsg {
    /// The page's effects are unscoped (never wrapped in `Msg::Scoped`), so nothing arrives for
    /// an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ShowPage
            | Self::Reload
            | Self::CopyPublicKey(_)
            | Self::Generate { .. }
            | Self::UseKey { .. }
            | Self::StopUsing { .. }
            | Self::SetUseAgent(_)
            | Self::ReloadAgent
            | Self::AddToAgent(_)
            | Self::ChooseService { .. }
            | Self::SendToService
            | Self::CancelService
            | Self::OpenKeySettings { .. }
            | Self::Loaded { .. }
            | Self::Generated(_)
            | Self::ChoiceLoaded { .. }
            | Self::KeyChoiceSet { .. }
            | Self::AgentLoaded { .. }
            | Self::AgentAdded { .. }
            | Self::AddedToService { .. } => None,
        }
    }
}

/// The page's slow work, run outside any repository. `Debug` hides the passphrase (through
/// `NewKey`).
#[derive(Debug)]
pub enum SshEffect {
    /// Read ~/.ssh: the key pairs, their fingerprints and whether ssh-keygen is installed.
    List { generation: u64 },
    /// Run ssh-keygen to make `key`; without `overwrite`, an existing key there is refused.
    Generate { keygen: PathBuf, key: NewKey, overwrite: bool },
    /// Read core.sshCommand globally and, with `repo`, in that repository.
    ReadChoice { generation: u64, repo: Option<PathBuf> },
    /// Write (`key`) or unset core.sshCommand globally, or in `repo`.
    SetKeyChoice { repo: Option<PathBuf>, key: Option<PathBuf> },
    /// Check for a running agent and what it holds.
    Agent { generation: u64 },
    /// `ssh-add <key>`, prompting for a passphrase through the askpass dialog.
    AddToAgent { ssh_add: PathBuf, key: PathBuf },
    /// Add a public key to a hosting-service account (signing in first).
    AddToService { account: Account, public_key: String, title: String, key: PathBuf },
}

/// What an `SshEffect` reports if it can't finish (see `runner/guard.rs`).
pub enum SshFailure {
    List(u64),
    Generate,
    ReadChoice(u64),
    SetKeyChoice { repo: bool },
    Agent(u64),
    AddToAgent(PathBuf),
    AddToService { key: PathBuf, account: Account },
}

impl SshEffect {
    pub fn name(&self) -> &'static str {
        match self {
            SshEffect::List { .. } => "ssh-keys",
            SshEffect::Generate { .. } => "ssh-keygen",
            SshEffect::ReadChoice { .. } => "ssh-choice",
            SshEffect::SetKeyChoice { .. } => "ssh-config",
            SshEffect::Agent { .. } => "ssh-agent",
            SshEffect::AddToAgent { .. } => "ssh-add",
            SshEffect::AddToService { .. } => "ssh-key-upload",
        }
    }

    pub fn failure(&self) -> SshFailure {
        match self {
            SshEffect::List { generation } => SshFailure::List(*generation),
            SshEffect::Generate { .. } => SshFailure::Generate,
            SshEffect::ReadChoice { generation, .. } => SshFailure::ReadChoice(*generation),
            SshEffect::SetKeyChoice { repo, .. } => SshFailure::SetKeyChoice { repo: repo.is_some() },
            SshEffect::Agent { generation } => SshFailure::Agent(*generation),
            SshEffect::AddToAgent { key, .. } => SshFailure::AddToAgent(key.clone()),
            SshEffect::AddToService { key, account, .. } => SshFailure::AddToService { key: key.clone(), account: account.clone() },
        }
    }
}

impl SshFailure {
    /// The failed result the effect's sender is waiting for.
    pub fn message(self, text: String) -> SshMsg {
        match self {
            SshFailure::List(generation) => SshMsg::Loaded { generation, result: Err(text) },
            SshFailure::Generate => SshMsg::Generated(Err(GenerateFailure::Failed(text))),
            SshFailure::ReadChoice(generation) => SshMsg::ChoiceLoaded { generation, global: Err(text), repo: None },
            SshFailure::SetKeyChoice { repo } => SshMsg::KeyChoiceSet { repo, result: Err(text) },
            SshFailure::Agent(generation) => {
                SshMsg::AgentLoaded { generation, status: AgentStatus { reach: Some(AgentReach::Failed(text)), ..Default::default() } }
            }
            SshFailure::AddToAgent(key) => SshMsg::AgentAdded { key, result: Err(text) },
            SshFailure::AddToService { key, account } => {
                SshMsg::AddedToService { key, account, result: Err(ServiceKeyError { message: text, fallback: false }) }
            }
        }
    }
}

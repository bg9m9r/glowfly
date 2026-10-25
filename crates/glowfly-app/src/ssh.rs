//! Messages and effects of Preferences > SSH: the key pairs in ~/.ssh and the ones added with Use
//! existing (picked in the OS file dialog), generating a key in its dialog, the key git uses
//! (core.sshCommand, and on Windows pointing git at Windows' OpenSSH), the keys ~/.ssh/config
//! names and the ones only the agent holds, the local SSH agent and adding a key to a hosting
//! service (state/ssh*.rs, update/ssh*.rs, runner/ssh.rs, ui/prefs/ssh*.rs,
//! ui/ssh_keygen_dialog.rs).

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use crate::msg::Inactive;
use glowfly_core::forge::{Account, AddKeyOutcome};
use glowfly_core::ssh_keys::{AgentReach, AgentStatus, KeyChoice, KeyFileProblem, NewKey, SshKeyPair, SshListing};
use std::path::{Path, PathBuf};

pub enum SshMsg {
    /// The navigation row: switch Preferences to the page and read ~/.ssh.
    ShowPage,
    /// Read ~/.ssh again (the Refresh button).
    Reload,
    /// Copy the public key of the listed pair whose private key is at this path.
    CopyPublicKey(PathBuf),
    /// The Generate… button: open the Generate SSH key dialog.
    OpenGenerate,
    /// The dialog's Generate key button, or accepting the Replace confirmation (`overwrite`).
    Generate {
        overwrite: bool,
    },
    /// The Use existing… button: pick a key pair outside the list in the OS file dialog.
    UseExisting,
    /// Remove from list: forget a key added with Use existing (its files stay).
    ForgetKey(PathBuf),
    /// Which key: make git use this key for SSH remotes, for every repository or (`repo`)
    /// the open one; `confirmed` once the user agreed to replace a command Glowfly didn't write.
    UseKey {
        key: PathBuf,
        repo: bool,
        confirmed: bool,
    },
    /// Let ssh decide: unset core.sshCommand at one level.
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
    /// Copy the public key of the agent-only key with this fingerprint.
    CopyAgentKey(String),
    /// Use Windows' OpenSSH (true) or Use Git's own ssh (false): global core.sshCommand.
    UseWindowsSsh(bool),
    // ---- from the runner
    Loaded {
        generation: u64,
        result: Result<SshListing, String>,
    },
    Generated(Result<SshKeyPair, GenerateFailure>),
    /// The file the dialog picked, read as a key pair; None when it was cancelled.
    KeyPicked(Option<(PathBuf, Result<SshKeyPair, KeyFileProblem>)>),
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
    /// The Windows' OpenSSH switch (`on`) or its undo finished.
    WindowsSshSet {
        on: bool,
        result: Result<(), String>,
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
            | Self::OpenGenerate
            | Self::Generate { .. }
            | Self::UseExisting
            | Self::ForgetKey(_)
            | Self::KeyPicked(_)
            | Self::UseKey { .. }
            | Self::StopUsing { .. }
            | Self::SetUseAgent(_)
            | Self::ReloadAgent
            | Self::AddToAgent(_)
            | Self::ChooseService { .. }
            | Self::SendToService
            | Self::CancelService
            | Self::OpenKeySettings { .. }
            | Self::CopyAgentKey(_)
            | Self::UseWindowsSsh(_)
            | Self::WindowsSshSet { .. }
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
    /// Read ~/.ssh and the `extra` keys added with Use existing: the key pairs, their
    /// fingerprints and whether ssh-keygen is installed.
    List { generation: u64, extra: Vec<PathBuf> },
    /// Ask for a private key in the OS file dialog, starting in `start`, and read the pair.
    PickKey { title: String, start: PathBuf, keygen: Option<PathBuf> },
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
    /// Make git run Windows' OpenSSH `program` for every repository (global core.sshCommand),
    /// or with None unset that value again.
    SetWindowsSsh { program: Option<PathBuf> },
}

/// What an `SshEffect` reports if it can't finish (see `runner/guard.rs`).
pub enum SshFailure {
    List(u64),
    PickKey,
    Generate,
    ReadChoice(u64),
    SetKeyChoice { repo: bool },
    Agent(u64),
    AddToAgent(PathBuf),
    AddToService { key: PathBuf, account: Account },
    SetWindowsSsh { on: bool },
}

impl SshEffect {
    pub fn failure(&self) -> SshFailure {
        match self {
            SshEffect::List { generation, .. } => SshFailure::List(*generation),
            SshEffect::PickKey { .. } => SshFailure::PickKey,
            SshEffect::Generate { .. } => SshFailure::Generate,
            SshEffect::ReadChoice { generation, .. } => SshFailure::ReadChoice(*generation),
            SshEffect::SetKeyChoice { repo, .. } => SshFailure::SetKeyChoice { repo: repo.is_some() },
            SshEffect::Agent { generation } => SshFailure::Agent(*generation),
            SshEffect::AddToAgent { key, .. } => SshFailure::AddToAgent(key.clone()),
            SshEffect::AddToService { key, account, .. } => SshFailure::AddToService { key: key.clone(), account: account.clone() },
            SshEffect::SetWindowsSsh { program } => SshFailure::SetWindowsSsh { on: program.is_some() },
        }
    }
}

impl EffectMeta for SshEffect {
    fn name(&self) -> &'static str {
        match self {
            SshEffect::List { .. } => "ssh-keys",
            SshEffect::PickKey { .. } => "ssh-pick-key",
            SshEffect::Generate { .. } => "ssh-keygen",
            SshEffect::ReadChoice { .. } => "ssh-choice",
            SshEffect::SetKeyChoice { .. } => "ssh-config",
            SshEffect::Agent { .. } => "ssh-agent",
            SshEffect::AddToAgent { .. } => "ssh-add",
            SshEffect::AddToService { .. } => "ssh-key-upload",
            SshEffect::SetWindowsSsh { .. } => "ssh-windows",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Whether the runner times this for the performance log. `ssh-add` waits while the user
    /// types the key's passphrase, the upload may wait on a sign-in window, and the file dialog
    /// on the user.
    fn timed(&self) -> bool {
        match self {
            SshEffect::List { .. }
            | SshEffect::Generate { .. }
            | SshEffect::ReadChoice { .. }
            | SshEffect::SetKeyChoice { .. }
            | SshEffect::Agent { .. }
            | SshEffect::SetWindowsSsh { .. } => true,
            SshEffect::PickKey { .. } | SshEffect::AddToAgent { .. } | SshEffect::AddToService { .. } => false,
        }
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}

impl SshFailure {
    /// The failed result the effect's sender is waiting for.
    pub fn message(self, text: String) -> SshMsg {
        match self {
            SshFailure::List(generation) => SshMsg::Loaded { generation, result: Err(text) },
            SshFailure::PickKey => SshMsg::KeyPicked(Some((PathBuf::new(), Err(KeyFileProblem::Unreadable(text))))),
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
            SshFailure::SetWindowsSsh { on } => SshMsg::WindowsSshSet { on, result: Err(text) },
        }
    }
}

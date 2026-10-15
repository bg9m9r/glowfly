//! Messages and effects of Preferences > SSH: the key pairs in ~/.ssh and generating a key
//! (state/ssh.rs, update/ssh.rs, runner/ssh.rs, ui/prefs/ssh.rs).

use crate::msg::Inactive;
use std::path::PathBuf;
use trellis_core::ssh_keys::{NewKey, SshKeyPair, SshListing};

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
    // ---- from the runner
    Loaded {
        generation: u64,
        result: Result<SshListing, String>,
    },
    Generated(Result<SshKeyPair, GenerateFailure>),
}

/// Why a key wasn't generated.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerateFailure {
    /// A key (or its .pub) is already at this path: ask before replacing it.
    Exists(PathBuf),
    Failed(String),
}

impl SshMsg {
    /// The page's effects are unscoped (never wrapped in `Msg::Scoped`), so nothing arrives for
    /// an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ShowPage | Self::Reload | Self::CopyPublicKey(_) | Self::Generate { .. } | Self::Loaded { .. } | Self::Generated(_) => {
                None
            }
        }
    }
}

/// Listing and generating keys, run outside any repository. `Debug` hides the passphrase
/// (through `NewKey`).
#[derive(Debug)]
pub enum SshEffect {
    /// Read ~/.ssh: the key pairs, their fingerprints and whether ssh-keygen is installed.
    List { generation: u64 },
    /// Run ssh-keygen to make `key`; without `overwrite`, an existing key there is refused.
    Generate { keygen: PathBuf, key: NewKey, overwrite: bool },
}

impl SshEffect {
    pub fn name(&self) -> &'static str {
        match self {
            SshEffect::List { .. } => "ssh-keys",
            SshEffect::Generate { .. } => "ssh-keygen",
        }
    }
}

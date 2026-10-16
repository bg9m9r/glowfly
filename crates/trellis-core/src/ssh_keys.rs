//! SSH key pairs in `~/.ssh` and how git reaches SSH remotes with them. Trellis keeps no key
//! material; it reads the files ssh uses and runs ssh's own tools.
//!
//! - `listing`: the pairs in a folder, their public key lines and `ssh-keygen` fingerprints
//! - `tools`: finding `ssh-keygen` and `ssh-add`
//! - `generate`: generating Ed25519 keys with `ssh-keygen`
//! - `command`: the key git uses, written into `core.sshCommand`
//! - `agent`: finding a running SSH agent and adding keys to it with `ssh-add`

use crate::error::Result;
use std::path::{Path, PathBuf};

mod agent;
mod command;
mod generate;
mod listing;
mod tools;

pub use agent::{
    AgentKey, AgentReach, AgentStatus, WinService, add_args, add_to_agent, agent_status, is_pageant_pipe, parse_agent_line,
    parse_agent_list, parse_sc_query, sc_start_disabled, ssh_add_candidates_for,
};
pub use command::{
    KeyChoice, SSH_COMMAND_KEY, parse_ssh_command, read_choice, split_sh_words, ssh_command_for, ssh_command_for_os, write_choice,
};
pub use generate::{
    LocationProblem, NewKey, comment_problem, default_location, generate, keygen_args, resolve_location, resolve_location_for,
};
pub use listing::{fingerprint, key_body, kind_from_type, list_key_pairs, parse_fingerprint, parse_public_key};
pub use tools::{find_keygen, find_ssh_add, keygen_candidates, tool_candidates};

#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod generate_tests;
#[cfg(test)]
mod tests;

/// A private key in the .ssh folder with a matching, readable `.pub` file next to it.
#[derive(Clone, Debug, PartialEq)]
pub struct SshKeyPair {
    pub private: PathBuf,
    pub public: PathBuf,
    /// The first token of the `.pub` line, e.g. "ssh-ed25519".
    pub key_type: String,
    /// "ED25519", "RSA", "ECDSA", "ED25519-SK", …
    pub kind: String,
    pub bits: Option<u32>,
    /// "SHA256:…", from `ssh-keygen -l`; None when it couldn't be read.
    pub fingerprint: Option<String>,
    pub comment: String,
    /// The `.pub` line, trimmed: what Copy public key puts on the clipboard.
    pub public_key: String,
}

impl SshKeyPair {
    pub fn file_name(&self) -> String {
        self.private.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// The kind, with the size for key types that come in several sizes ("RSA 3072").
    pub fn kind_label(&self) -> String {
        match self.bits {
            Some(bits) if matches!(self.kind.as_str(), "RSA" | "ECDSA" | "DSA") => format!("{} {bits}", self.kind),
            _ => self.kind.clone(),
        }
    }
}

/// What Preferences > SSH shows: the key pairs and whether ssh-keygen was found.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SshListing {
    pub home: PathBuf,
    /// home/.ssh
    pub dir: PathBuf,
    pub keys: Vec<SshKeyPair>,
    pub keygen: Option<PathBuf>,
}

pub fn ssh_dir(home: &Path) -> PathBuf {
    home.join(".ssh")
}

/// The public key file of `private`: the same path with `.pub` added.
pub(crate) fn pub_path(private: &Path) -> PathBuf {
    let mut s = private.as_os_str().to_owned();
    s.push(".pub");
    PathBuf::from(s)
}

/// Everything Preferences > SSH shows for the user whose home folder is `home`.
pub fn list(home: &Path, cwd: &Path) -> Result<SshListing> {
    let keygen = find_keygen(cwd);
    let dir = ssh_dir(home);
    let keys = list_key_pairs(&dir, keygen.as_deref())?;
    Ok(SshListing { home: home.to_owned(), dir, keys, keygen })
}

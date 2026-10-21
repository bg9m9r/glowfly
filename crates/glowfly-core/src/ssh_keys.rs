//! SSH key pairs in `~/.ssh` and how git reaches SSH remotes with them. Trellis keeps no key
//! material; it reads the files ssh uses and runs ssh's own tools.
//!
//! - `listing`: the pairs in a folder, their public key lines and `ssh-keygen` fingerprints, a
//!   pair picked anywhere (Use existing) and the key names ssh tries by default
//! - `tools`: finding `ssh-keygen` and `ssh-add`
//! - `generate`: generating Ed25519 keys with `ssh-keygen`
//! - `command`: the key git uses, written into `core.sshCommand`
//! - `agent`: finding a running SSH agent and adding keys to it with `ssh-add`
//! - `config`: splitting `~/.ssh/config` lines as ssh does and picking out `Host`, `Match`,
//!   `IdentityFile` and `Include`
//! - `config_expand`: expanding an `IdentityFile` value (`~`, `%` tokens, `${VAR}`) and deciding
//!   whether it is a full path by each OS's rules
//! - `config_scan`: reading `~/.ssh/config` and the files it Includes for their IdentityFile
//!   entries
//! - `windows_ssh`: which ssh git runs on Windows, and pointing it at Windows' own OpenSSH

use crate::error::Result;
use std::path::{Path, PathBuf};

mod agent;
mod command;
mod config;
mod config_expand;
mod config_scan;
mod generate;
mod listing;
mod tools;
mod windows_ssh;

pub use agent::{
    AgentKey, AgentReach, AgentStatus, WinService, add_args, add_to_agent, agent_status, attach_public_keys, is_pageant_pipe, pair_matches,
    parse_agent_line, parse_agent_list, parse_sc_query, sc_start_disabled, ssh_add_candidates_for,
};
pub use command::{
    KeyChoice, SSH_COMMAND_KEY, parse_ssh_command, read_choice, split_sh_words, ssh_command_for, ssh_command_for_os, write_choice,
};
pub use config::{Directive, config_words, parse_config};
pub use config_expand::{ExpandCx, Unresolved, absolute_for, expand_identity, user_name_for};
pub use config_scan::{ConfigScan, ScannedIdentity, name_matches, scan_config};
pub use generate::{
    LocationProblem, NewKey, comment_problem, default_location, generate, keygen_args, resolve_location, resolve_location_for,
};
pub use listing::{
    DEFAULT_IDENTITY_NAMES, KeyFileProblem, fingerprint, is_default_identity, key_body, kind_from_type, list_key_pairs, parse_fingerprint,
    parse_public_key, public_fingerprint, read_key_pair,
};
pub use tools::{find_keygen, find_ssh_add, keygen_candidates, tool_candidates};
pub use windows_ssh::{
    GitSsh, WinSshFacts, facts_from, git_runs, is_windows_ssh_word, program_word, windows_ssh_command_for, write_windows_ssh,
};

#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod config_scan_tests;
#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod generate_tests;
#[cfg(test)]
mod list_config_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod windows_ssh_tests;

/// A private key (in the .ssh folder, or added with Use existing) with a matching, readable
/// `.pub` file next to it.
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

/// What Preferences > SSH shows: the key pairs (the .ssh folder's, then the ones
/// `~/.ssh/config` names, then the extra ones), whether ssh-keygen was found, and every
/// IdentityFile entry of the config with what it points at.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SshListing {
    pub home: PathBuf,
    /// home/.ssh
    pub dir: PathBuf,
    pub keys: Vec<SshKeyPair>,
    pub keygen: Option<PathBuf>,
    /// home/.ssh/config
    pub config_file: PathBuf,
    /// The IdentityFile entries of the config and the files it Includes, in the order ssh reads
    /// them.
    pub config: Vec<ConfigIdentity>,
    /// Why the config couldn't be read (a missing one is no error).
    pub config_error: Option<String>,
}

/// One IdentityFile entry of `~/.ssh/config` (or a file it Includes) and what it points at.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigIdentity {
    /// The value as written.
    pub raw: String,
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
    /// The `Host` or `Match` line it is under ("" at the top of the file).
    pub section: String,
    pub status: ConfigStatus,
}

/// What an IdentityFile entry points at.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigStatus {
    /// A listed key pair, by its private key.
    Key(PathBuf),
    /// Only `<path>.pub` is there; ssh uses it to pick that key from the agent.
    PublicOnly(PathBuf),
    /// Nothing is there.
    Missing(PathBuf),
    /// Something is there, but not a pair the list can show.
    NotAPair(PathBuf, KeyFileProblem),
    Unresolved(Unresolved),
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

/// Everything Preferences > SSH shows for the user whose home folder is `home` (see
/// [`list_with`]), with ssh-keygen looked up from `cwd` and the local user from the environment.
pub fn list(home: &Path, cwd: &Path, extra: &[PathBuf]) -> Result<SshListing> {
    let keygen = find_keygen(cwd);
    let (user, uid) = config_expand::from_env();
    let env = |name: &str| std::env::var(name).ok();
    let cx = ExpandCx { home, user: &user, uid, env: &env, windows: cfg!(windows) };
    list_with(home, keygen.as_deref(), extra, &cx)
}

/// The pairs in `home`'s .ssh folder, sorted by name; then the pairs `~/.ssh/config` names, in
/// its order, each once; then each of `extra` (keys added with Use existing) that still reads
/// and isn't listed already, in the order given. Extras that fail are skipped; config entries
/// that aren't pairs are kept in `config` with the reason.
pub fn list_with(home: &Path, keygen: Option<&Path>, extra: &[PathBuf], cx: &ExpandCx) -> Result<SshListing> {
    let dir = ssh_dir(home);
    let mut keys = list_key_pairs(&dir, keygen)?;
    let scan = scan_config(&dir, cx);
    let config = scan
        .entries
        .into_iter()
        .map(|e| {
            let status = match e.path {
                Err(u) => ConfigStatus::Unresolved(u),
                Ok(path) => config_status(path, keygen, &mut keys),
            };
            ConfigIdentity { raw: e.raw, file: e.file, line: e.line, section: e.section, status }
        })
        .collect();
    for path in extra {
        if keys.iter().any(|k| crate::paths::same_folder(&k.private, path)) {
            continue;
        }
        if let Ok(pair) = read_key_pair(path, keygen) {
            keys.push(pair);
        }
    }
    Ok(SshListing {
        home: home.to_owned(),
        config_file: dir.join("config"),
        dir,
        keys,
        keygen: keygen.map(Path::to_owned),
        config,
        config_error: scan.error,
    })
}

/// What the file an IdentityFile entry names is, adding its pair to `keys` unless it is listed.
/// A listed key is matched by asking the disk (`same_folder`), so a link, a short 8.3 name or a
/// case difference on a volume that ignores case still finds it; the status then names the key
/// as listed.
fn config_status(path: PathBuf, keygen: Option<&Path>, keys: &mut Vec<SshKeyPair>) -> ConfigStatus {
    use crate::paths::same_folder;
    let listed = |keys: &[SshKeyPair], private: &Path| keys.iter().find(|k| same_folder(&k.private, private)).map(|k| k.private.clone());
    if let Some(private) = listed(keys, &path) {
        return ConfigStatus::Key(private);
    }
    // A public key named alone, with no private key next to it.
    if path.extension().is_some_and(|x| x == "pub") && path.is_file() && !path.with_extension("").is_file() {
        return ConfigStatus::PublicOnly(path.with_extension(""));
    }
    match read_key_pair(&path, keygen) {
        Ok(pair) => match listed(keys, &pair.private) {
            Some(private) => ConfigStatus::Key(private),
            None => {
                let private = pair.private.clone();
                keys.push(pair);
                ConfigStatus::Key(private)
            }
        },
        Err(KeyFileProblem::NotAFile) if pub_path(&path).is_file() => ConfigStatus::PublicOnly(path),
        Err(KeyFileProblem::NotAFile) if !path.exists() => ConfigStatus::Missing(path),
        Err(problem) => ConfigStatus::NotAPair(path, problem),
    }
}

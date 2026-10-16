//! Preferences > SSH: reading the key pairs in ~/.ssh (`SshEffect::List`), generating a key with
//! ssh-keygen, reading and writing core.sshCommand, checking the agent and adding a key to it,
//! and adding a public key to a hosting-service account.

use crate::msg::Msg;
use crate::ssh::{GenerateFailure, ServiceKeyError, SshEffect, SshMsg};
use std::path::PathBuf;
use trellis_core::error::Error;
use trellis_core::forge;
use trellis_core::signing::home_dir;
use trellis_core::ssh_keys;

/// Where global reads and writes run: the home folder, else the current folder.
fn home_or_here() -> PathBuf {
    home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub(super) fn run(e: SshEffect) -> Msg {
    Msg::Ssh(match e {
        SshEffect::List { generation } => {
            let result = match home_dir() {
                Some(home) => ssh_keys::list(&home, &home).map_err(|e| format!("Couldn't read your .ssh folder: {e}")),
                None => Err("Couldn't find your home folder.".into()),
            };
            SshMsg::Loaded { generation, result }
        }
        SshEffect::Generate { keygen, key, overwrite } => {
            let result = ssh_keys::generate(&keygen, &key, overwrite).map_err(|e| match e {
                Error::WouldOverwrite(_) => GenerateFailure::Exists(key.path.clone()),
                other => GenerateFailure::Failed(other.to_string()),
            });
            SshMsg::Generated(result)
        }
        SshEffect::ReadChoice { generation, repo } => {
            let global = ssh_keys::read_choice(&home_or_here(), true, &[]);
            let repo = repo.map(|workdir| ssh_keys::read_choice(&workdir, false, &[]));
            SshMsg::ChoiceLoaded { generation, global, repo }
        }
        SshEffect::SetKeyChoice { repo, key } => {
            let cwd = repo.clone().unwrap_or_else(home_or_here);
            let result = ssh_keys::write_choice(&cwd, repo.is_none(), key.as_deref(), &[]);
            SshMsg::KeyChoiceSet { repo: repo.is_some(), result }
        }
        SshEffect::Agent { generation } => SshMsg::AgentLoaded { generation, status: ssh_keys::agent_status(&home_or_here()) },
        SshEffect::AddToAgent { ssh_add, key } => {
            let result = ssh_keys::add_to_agent(&ssh_add, &key);
            SshMsg::AgentAdded { key, result }
        }
        SshEffect::AddToService { account, public_key, title, key } => {
            let result = forge::resolve_auth(&account, &home_or_here(), true, true)
                .and_then(|auth| forge::add_ssh_key(&account, &auth, &public_key, &title))
                .map_err(|e| ServiceKeyError { message: e.to_string(), fallback: forge::needs_fallback(&e) });
            SshMsg::AddedToService { key, account, result }
        }
    })
}

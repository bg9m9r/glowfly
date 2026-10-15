//! Preferences > SSH: reading the key pairs in ~/.ssh (`SshEffect::List`) and generating a key
//! with ssh-keygen (`SshEffect::Generate`).

use crate::msg::Msg;
use crate::ssh::{GenerateFailure, SshEffect, SshMsg};
use trellis_core::error::Error;
use trellis_core::signing::home_dir;
use trellis_core::ssh_keys;

pub(super) fn run(e: SshEffect) -> Msg {
    match e {
        SshEffect::List { generation } => {
            let result = match home_dir() {
                Some(home) => ssh_keys::list(&home, &home).map_err(|e| format!("Couldn't read your .ssh folder: {e}")),
                None => Err("Couldn't find your home folder.".into()),
            };
            Msg::Ssh(SshMsg::Loaded { generation, result })
        }
        SshEffect::Generate { keygen, key, overwrite } => {
            let result = ssh_keys::generate(&keygen, &key, overwrite).map_err(|e| match e {
                Error::WouldOverwrite(_) => GenerateFailure::Exists(key.path.clone()),
                other => GenerateFailure::Failed(other.to_string()),
            });
            Msg::Ssh(SshMsg::Generated(result))
        }
    }
}

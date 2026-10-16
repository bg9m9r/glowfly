//! Preferences > SSH: adding a public key to a connected hosting-service account, through the
//! inline form under the key's row.

use std::path::PathBuf;
use trellis_core::forge::{Account, Connection, title_problem};

/// The Add to service form for one key and account.
#[derive(Clone, Debug, PartialEq)]
pub struct UploadForm {
    pub key: PathBuf,
    pub account: Account,
    /// The Title field (a view text buffer); empty uses `default_title`.
    pub title: String,
    pub default_title: String,
    pub state: UploadState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UploadState {
    Editing,
    Sending,
    /// The account has the key already, under this title.
    AlreadyThere(String),
    Failed {
        message: String,
        /// The key was copied and the settings page opened instead.
        fallback: bool,
    },
}

impl UploadForm {
    pub fn new(key: PathBuf, account: Account, default_title: String) -> Self {
        UploadForm { key, account, title: String::new(), default_title, state: UploadState::Editing }
    }

    /// Why Add key can't run now.
    pub fn send_blocked(&self) -> Option<&'static str> {
        if self.state == UploadState::Sending {
            return Some("The key is being sent.");
        }
        title_problem(&self.title)
    }

    /// The title to send: the typed one, or the default.
    pub fn title_to_send(&self) -> String {
        let t = self.title.trim();
        if t.is_empty() { self.default_title.clone() } else { t.to_owned() }
    }

    pub fn service(&self) -> &'static str {
        self.account.provider.name()
    }
}

/// The connected code-hosting accounts a key can be added to (issue trackers left out).
pub fn services(connections: &[Connection]) -> Vec<&Connection> {
    connections.iter().filter(|c| !c.account.provider.is_tracker_only()).collect()
}

/// The Add to service menu's label for an account.
pub fn service_label(account: &Account) -> String {
    format!("{} ({})", account.provider.name(), account.display())
}

pub fn fallback_note(service: &str) -> String {
    format!(
        "{service} didn't let Trellis add keys with this sign-in (it needs permission to write SSH keys). The key is copied; paste it on the page that opened."
    )
}

pub fn already_note(service: &str, title: &str) -> String {
    format!("{service} already has this key (titled \u{201c}{title}\u{201d}).")
}

pub fn pasted_toast(service: &str) -> String {
    format!("Public key copied. Paste it on the {service} page that opened.")
}

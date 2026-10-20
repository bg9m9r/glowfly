//! Preferences > SSH: adding a public key to a connected hosting-service account, through the
//! inline form under the key's row.

use std::path::PathBuf;
use trellis_core::forge::{Account, Connection, title_problem};
use trellis_core::i18n::Lang;

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

    /// Why Add key can't run now, in `t`.
    pub fn send_blocked(&self, t: Lang) -> Option<&'static str> {
        if self.state == UploadState::Sending {
            return Some(t.tr("prefs.ssh.service.sending.tip"));
        }
        self.title_problem(t)
    }

    /// Why the typed title can't be used, in `t`.
    pub fn title_problem(&self, t: Lang) -> Option<&'static str> {
        title_problem(&self.title).map(|_| t.tr("prefs.ssh.service.title.control.warn"))
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

pub fn fallback_note(t: Lang, service: &str) -> String {
    t.trf("prefs.ssh.service.fallback.warn", &[("service", &service)])
}

pub fn already_note(t: Lang, service: &str, title: &str) -> String {
    t.trf("prefs.ssh.service.already.help", &[("service", &service), ("title", &title)])
}

pub fn pasted_toast(service: &str) -> String {
    format!("Public key copied. Paste it on the {service} page that opened.")
}

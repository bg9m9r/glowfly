//! The `Execute` impl of `TlsEffect`: reading the TLS settings git uses for a failed HTTPS job
//! (`glowfly_core::tls_config`, with `GIT_SSL_CAINFO` read from Glowfly's environment, never
//! changed) and setting `http.schannelCheckRevoke` to best-effort once confirmed.

use super::execute::{Cx, Execute};
use glowfly_core::error::ReadFailed;
use glowfly_core::tls_config;
use glowfly_model::effect::TlsEffect;
use glowfly_model::msg::{Msg, TlsMsg};

impl Execute for TlsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Tls(match self {
            TlsEffect::ReadSettings { cwd, url, entry, failure } => {
                let result = tls_config::read(&cwd, url.as_deref(), &[])
                    .map(|mut s| {
                        s.ca_info_env = std::env::var("GIT_SSL_CAINFO").ok().filter(|v| !v.is_empty());
                        Box::new(s)
                    })
                    .map_err(|said| ReadFailed::Said { said });
                TlsMsg::SettingsRead { entry, failure, result }
            }
            TlsEffect::SetBestEffort { cwd, global, retry } => {
                let result = glowfly_core::identity::set_config(&cwd, global, "http.schannelCheckRevoke", Some("best-effort"), &[]);
                TlsMsg::BestEffortSet { global, retry, result }
            }
        }))
    }
}

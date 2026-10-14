//! Credentials: environment variables, the OS credential store and Git Credential Manager.

use super::*;
use crate::error::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone)]
pub struct Auth {
    pub(super) header: String,
    pub source: String,
}

/// Never prints the credential itself.
impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth").field("header", &"<redacted>").field("source", &self.source).finish()
    }
}

impl Auth {
    fn bearer(token: &str, source: &str) -> Auth {
        Auth { header: format!("Bearer {}", token.trim()), source: source.into() }
    }

    fn basic(user: &str, pass: &str, source: &str) -> Auth {
        let b = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{}", pass.trim()));
        Auth { header: format!("Basic {b}"), source: source.into() }
    }

    /// Builds the right header for a token (and optional username) on a given service.
    pub fn from_token(provider: Provider, user: Option<&str>, token: &str, source: &str) -> Auth {
        match (provider, user.filter(|u| !u.is_empty())) {
            (Provider::AzureDevOps, _) => Auth::basic("", token, source),
            (Provider::JiraCloud, Some(email)) => Auth::basic(email, token, source),
            (Provider::Bitbucket, Some(u)) => Auth::basic(u, token, source),
            _ => Auth::bearer(token, source),
        }
    }
}

const KEYRING_SERVICE: &str = "trellis";
/// Where tokens were saved while the app was called Riffle; moved on first use.
const OLD_KEYRING_SERVICE: &str = "riffle";

#[derive(Serialize, Deserialize)]
struct Secret {
    #[serde(default)]
    user: Option<String>,
    token: String,
}

fn keyring_entry(account: &Account) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, &account.key())?)
}

fn old_keyring_entry(account: &Account) -> Option<keyring::Entry> {
    keyring::Entry::new(OLD_KEYRING_SERVICE, &account.key()).ok()
}

pub fn save_token(account: &Account, user: Option<&str>, token: &str) -> Result<()> {
    let secret = Secret { user: user.map(str::to_owned), token: token.trim().to_owned() };
    keyring_entry(account)?.set_password(&serde_json::to_string(&secret)?)?;
    Ok(())
}

pub fn delete_token(account: &Account) -> Result<()> {
    if let Some(old) = old_keyring_entry(account) {
        let _ = old.delete_credential();
    }
    match keyring_entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn saved_token(account: &Account) -> Option<Secret> {
    let entry = keyring_entry(account).ok()?;
    let raw = match entry.get_password() {
        Ok(raw) => raw,
        // Saved under the old name: copy it across, then remove the old entry.
        Err(keyring::Error::NoEntry) => {
            let old = old_keyring_entry(account)?;
            let raw = old.get_password().ok()?;
            if entry.set_password(&raw).is_ok() {
                let _ = old.delete_credential();
            }
            raw
        }
        Err(_) => return None,
    };
    serde_json::from_str(&raw).ok()
}

/// Finds credentials, in order: the provider's environment variable, a token saved in the OS
/// credential store, then Git Credential Manager (the same sign-in `git push` uses). GCM may
/// show its sign-in window only when `interactive` is set.
pub fn resolve_auth(account: &Account, cwd: &Path, interactive: bool) -> Result<Auth> {
    if let Some(var) = account.provider.env_var() {
        let alt = if var == "GITHUB_TOKEN" { std::env::var("GH_TOKEN").ok() } else { None };
        if let Some(token) = std::env::var(var).ok().or(alt).filter(|t| !t.trim().is_empty()) {
            return Ok(Auth::from_token(account.provider, None, &token, var));
        }
    }
    if let Some(s) = saved_token(account) {
        return Ok(Auth::from_token(account.provider, s.user.as_deref(), &s.token, "saved token"));
    }
    if account.provider.supports_gcm() {
        return from_credential_manager(account, cwd, interactive);
    }
    Err(Error::Auth(format!("not connected to {}", account.provider.name())))
}

pub(super) fn credential_host(account: &Account) -> String {
    match account.provider {
        Provider::AzureDevOps => azure::credential_host(account),
        _ => account.host.clone(),
    }
}

fn credential_request(account: &Account) -> String {
    let mut input = format!("protocol=https\nhost={}\n", credential_host(account));
    if !account.org.is_empty() {
        input.push_str(&format!("path={}\n", account.org));
    }
    input.push('\n');
    input
}

/// Asks Git Credential Manager (or whichever git credential helper is configured) for a token.
pub fn from_credential_manager(account: &Account, cwd: &Path, interactive: bool) -> Result<Auth> {
    let mut env = vec![];
    if !interactive {
        env.push(("GCM_INTERACTIVE", "never"));
    }
    let (ok, out) = crate::gitcli::run_with_input(cwd, &crate::gitcli::args(&["credential", "fill"]), &credential_request(account), &env);
    if !ok {
        return Err(Error::Auth(format!("not signed in to {}", account.provider.name())));
    }
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(str::to_owned);
    let password = field("password").ok_or_else(|| Error::Auth("the credential helper returned no token".into()))?;
    let user = field("username").unwrap_or_default();
    let source = "Git Credential Manager";
    Ok(match account.provider {
        // Microsoft Entra access tokens are JWTs; anything else is a PAT used with Basic auth.
        Provider::AzureDevOps if password.starts_with("eyJ") => Auth::bearer(&password, source),
        Provider::AzureDevOps => Auth::basic(&user, &password, source),
        // Bitbucket app passwords pair with the username; OAuth tokens are bearer tokens.
        Provider::Bitbucket if password.starts_with("ATBB") || password.len() <= 24 => Auth::basic(&user, &password, source),
        Provider::BitbucketDataCenter if password.len() < 30 => Auth::basic(&user, &password, source),
        _ => Auth::bearer(&password, source),
    })
}

/// Removes the sign-in Git Credential Manager stored for an account's host. Note this also
/// signs git itself out of that host.
pub fn forget_credential_manager(account: &Account, cwd: &Path) -> Result<()> {
    let (ok, out) = crate::gitcli::run_with_input(cwd, &crate::gitcli::args(&["credential", "reject"]), &credential_request(account), &[]);
    if ok { Ok(()) } else { Err(Error::Other(format!("git credential reject failed: {out}"))) }
}

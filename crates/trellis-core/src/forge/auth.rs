//! Credentials: environment variables, the OS credential store and Git Credential Manager.

use super::*;
use crate::error::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone)]
pub struct Auth {
    pub(super) header: String,
    /// The service the credential belongs to, which decides its service-specific headers.
    pub(super) provider: Provider,
    pub source: String,
}

/// Never prints the credential itself.
impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth").field("header", &"<redacted>").field("provider", &self.provider).field("source", &self.source).finish()
    }
}

impl Auth {
    fn bearer(provider: Provider, token: &str, source: &str) -> Auth {
        Auth { header: format!("Bearer {}", token.trim()), provider, source: source.into() }
    }

    fn basic(provider: Provider, user: &str, pass: &str, source: &str) -> Auth {
        let b = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{}", pass.trim()));
        Auth { header: format!("Basic {b}"), provider, source: source.into() }
    }

    /// Builds the right header for a token (and optional username) on a given service.
    pub fn from_token(provider: Provider, user: Option<&str>, token: &str, source: &str) -> Auth {
        match (provider, user.filter(|u| !u.is_empty())) {
            (Provider::AzureDevOps, _) => Auth::basic(provider, "", token, source),
            (Provider::JiraCloud, Some(email)) => Auth::basic(provider, email, token, source),
            (Provider::Bitbucket, Some(u)) => Auth::basic(provider, u, token, source),
            _ => Auth::bearer(provider, token, source),
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

/// Hosts run by the hosting service itself rather than self-hosted servers. Only these get
/// tokens from environment variables, and only these are contacted without a saved connection.
pub fn is_public_host(account: &Account) -> bool {
    let h = account.host.as_str();
    match account.provider {
        Provider::GitHub | Provider::GitLab | Provider::Bitbucket => account.provider.fixed_host() == Some(h),
        Provider::AzureDevOps => h == "dev.azure.com" || h.ends_with(".visualstudio.com"),
        _ => false,
    }
}

/// Whether Trellis may call this account's API: a public host, or one the user connected in
/// Preferences. A self-hosted server guessed from a remote URL alone is never contacted.
pub fn may_contact(account: &Account, connections: &[Connection]) -> bool {
    is_public_host(account) || is_connected(account, connections)
}

pub fn is_connected(account: &Account, connections: &[Connection]) -> bool {
    connections.iter().any(|c| c.account == *account)
}

/// The provider's environment-variable token, for public hosts only: a token meant for
/// github.com must never reach a server that merely calls itself `github.<something>`.
fn env_token(account: &Account, var: impl Fn(&str) -> Option<String>) -> Option<(String, &'static str)> {
    let name = account.provider.env_var().filter(|_| is_public_host(account))?;
    let found = |n: &'static str| var(n).filter(|t| !t.trim().is_empty()).map(|t| (t, n));
    found(name).or_else(|| if name == "GITHUB_TOKEN" { found("GH_TOKEN") } else { None })
}

/// Git Credential Manager is asked only for public hosts and hosts the user connected.
fn may_ask_gcm(account: &Account, connected: bool) -> bool {
    account.provider.supports_gcm() && (connected || is_public_host(account))
}

/// Finds credentials, in order: the provider's environment variable (public hosts only), a
/// token saved in the OS credential store, then Git Credential Manager (the same sign-in
/// `git push` uses; public hosts, or self-hosted ones the user `connected`). GCM may show its
/// sign-in window only when `interactive` is set.
pub fn resolve_auth(account: &Account, cwd: &Path, interactive: bool, connected: bool) -> Result<Auth> {
    if let Some((token, var)) = env_token(account, |n| std::env::var(n).ok()) {
        return Ok(Auth::from_token(account.provider, None, &token, var));
    }
    if let Some(s) = saved_token(account) {
        return Ok(Auth::from_token(account.provider, s.user.as_deref(), &s.token, "saved token"));
    }
    if may_ask_gcm(account, connected) {
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
    // Sign-in never opens Trellis's own credential prompt; GCM shows its own window.
    let mut env = crate::gitcli::NO_PROMPTS.to_vec();
    if !interactive {
        env.push(("GCM_INTERACTIVE", "never"));
    }
    // No timeout: the helper may be waiting on the user's sign-in window. Only stdout holds the
    // answer; a helper's own messages go to stderr.
    let a = crate::gitcli::args(&["credential", "fill"]);
    let out = crate::gitcli::output(cwd, &a, &credential_request(account), &env, None);
    if !out.ok {
        return Err(Error::Auth(format!("not signed in to {}", account.provider.name())));
    }
    let field = |k: &str| out.stdout.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(str::to_owned);
    let password = field("password").ok_or_else(|| Error::Auth("the credential helper returned no token".into()))?;
    let user = field("username").unwrap_or_default();
    let (p, source) = (account.provider, "Git Credential Manager");
    Ok(match p {
        // Microsoft Entra access tokens are JWTs; anything else is a PAT used with Basic auth.
        Provider::AzureDevOps if password.starts_with("eyJ") => Auth::bearer(p, &password, source),
        Provider::AzureDevOps => Auth::basic(p, &user, &password, source),
        // Bitbucket app passwords pair with the username; OAuth tokens are bearer tokens.
        Provider::Bitbucket if password.starts_with("ATBB") || password.len() <= 24 => Auth::basic(p, &user, &password, source),
        Provider::BitbucketDataCenter if password.len() < 30 => Auth::basic(p, &user, &password, source),
        _ => Auth::bearer(p, &password, source),
    })
}

/// Removes the sign-in Git Credential Manager stored for an account's host. Note this also
/// signs git itself out of that host.
pub fn forget_credential_manager(account: &Account, cwd: &Path) -> Result<()> {
    let (ok, out) = crate::gitcli::run_with_input(
        cwd,
        &crate::gitcli::args(&["credential", "reject"]),
        &credential_request(account),
        crate::gitcli::NO_PROMPTS,
    );
    if ok { Ok(()) } else { Err(Error::Other(format!("git credential reject failed: {out}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(provider: Provider, host: &str) -> Account {
        Account { provider, host: host.into(), org: String::new() }
    }

    fn env(name: &str) -> Option<String> {
        matches!(name, "GITHUB_TOKEN" | "GITLAB_TOKEN" | "GH_TOKEN").then(|| format!("secret-{name}"))
    }

    #[test]
    fn environment_tokens_go_to_public_hosts_only() {
        let gh = env_token(&acct(Provider::GitHub, "github.com"), env).unwrap();
        assert_eq!(gh, ("secret-GITHUB_TOKEN".to_owned(), "GITHUB_TOKEN"));
        assert!(env_token(&acct(Provider::GitLab, "gitlab.com"), env).is_some());
        let gh_only = |n: &str| (n == "GH_TOKEN").then(|| "t".to_owned());
        assert_eq!(env_token(&acct(Provider::GitHub, "github.com"), gh_only).unwrap().1, "GH_TOKEN");
        // Look-alike and self-hosted hosts never see an environment token.
        for a in [
            acct(Provider::GitHubEnterprise, "github.evil.example"),
            acct(Provider::GitLabSelfManaged, "gitlab.evil.example"),
            acct(Provider::GitLabSelfManaged, "mygitlab.corp"),
            acct(Provider::GitHub, "github.com.evil.example"),
        ] {
            assert_eq!(env_token(&a, env), None, "{a:?}");
        }
    }

    #[test]
    fn credential_manager_is_asked_only_for_public_or_connected_hosts() {
        assert!(may_ask_gcm(&acct(Provider::GitHub, "github.com"), false));
        assert!(may_ask_gcm(&acct(Provider::AzureDevOps, "dev.azure.com"), false));
        assert!(may_ask_gcm(&acct(Provider::AzureDevOps, "contoso.visualstudio.com"), false));
        let ghe = acct(Provider::GitHubEnterprise, "github.evil.example");
        assert!(!may_ask_gcm(&ghe, false));
        assert!(may_ask_gcm(&ghe, true));
        assert!(!may_ask_gcm(&acct(Provider::JiraCloud, "x.atlassian.net"), true), "Jira has no GCM sign-in");
    }

    #[test]
    fn only_public_or_connected_hosts_may_be_contacted() {
        let ghe = acct(Provider::GitHubEnterprise, "github.acme.io");
        assert!(!may_contact(&ghe, &[]));
        let conn = Connection { account: ghe.clone(), user: "ada".into(), method: "access token".into() };
        assert!(may_contact(&ghe, std::slice::from_ref(&conn)));
        // A connection to the same host under another provider doesn't count.
        assert!(!may_contact(&acct(Provider::GitLabSelfManaged, "github.acme.io"), &[conn]));
        assert!(may_contact(&acct(Provider::Bitbucket, "bitbucket.org"), &[]));
        assert!(!may_contact(&acct(Provider::BitbucketDataCenter, "bitbucket.acme.io"), &[]));
    }
}

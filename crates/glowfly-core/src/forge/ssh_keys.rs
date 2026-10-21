//! SSH public keys on the signed-in user's account: listing them, and adding one unless the
//! account already has it. GitHub (and Enterprise Server), GitLab and Bitbucket Cloud; the
//! other services get the key copied and their settings page opened instead (`ssh_keys_page`).

use super::{Account, Auth, Provider, enc, get, page, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;

const CLOUD_API: &str = "https://api.bitbucket.org/2.0";
/// The longest title Glowfly suggests.
const TITLE_CHARS: usize = 100;

/// A key the account already has.
#[derive(Clone, Debug, PartialEq)]
pub struct RegisteredKey {
    pub id: String,
    pub title: String,
    /// The public key line as the service shows it (type and body; the comment may be gone).
    pub key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AddKeyOutcome {
    Added {
        title: String,
    },
    /// The account had the key already, under this title; nothing was sent.
    AlreadyThere {
        title: String,
    },
}

/// Whether two public key lines are the same key: same type and body, whatever the comment.
pub fn same_key(a: &str, b: &str) -> bool {
    match (crate::ssh_keys::key_body(a), crate::ssh_keys::key_body(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// The title a key gets unless the user types one: its comment, else its file name.
pub fn default_title(comment: &str, file_name: &str) -> String {
    let comment = crate::text::printable(comment);
    let title = if comment.is_empty() { crate::text::printable(file_name) } else { comment };
    title.chars().take(TITLE_CHARS).collect()
}

/// Why `t` can't be a key title.
pub fn title_problem(t: &str) -> Option<&'static str> {
    t.chars().any(char::is_control).then_some("The title can't contain line breaks or control characters.")
}

/// The service's page for the user's SSH keys, for adding a key by hand.
pub fn ssh_keys_page(account: &Account) -> Option<String> {
    let host = &account.host;
    match account.provider {
        Provider::GitHub | Provider::GitHubEnterprise => Some(format!("https://{host}/settings/keys")),
        Provider::GitLab | Provider::GitLabSelfManaged => Some(format!("https://{host}/-/profile/keys")),
        Provider::Bitbucket => Some("https://bitbucket.org/account/settings/ssh-keys/".into()),
        Provider::BitbucketDataCenter => Some(format!("https://{host}/plugins/servlet/ssh/account/keys")),
        Provider::AzureDevOps if !account.org.is_empty() => {
            Some(format!("https://dev.azure.com/{}/_usersSettings/keys", enc(&account.org)))
        }
        Provider::AzureDevOps | Provider::JiraCloud | Provider::JiraDataCenter | Provider::Trello => None,
    }
}

/// An error after which the key should be copied and the settings page opened instead: the
/// sign-in may not write keys, or the service has no such API.
pub fn needs_fallback(e: &Error) -> bool {
    matches!(e, Error::Forbidden(_) | Error::NotFound(_) | Error::Unsupported(_))
}

fn unsupported(account: &Account) -> Error {
    Error::Unsupported(format!("Adding SSH keys to {} isn't supported", account.provider.name()))
}

/// GitHub's and GitLab's key entries.
#[derive(Deserialize)]
struct HubKey {
    id: u64,
    key: String,
    #[serde(default)]
    title: Option<String>,
}

impl From<HubKey> for RegisteredKey {
    fn from(k: HubKey) -> Self {
        RegisteredKey { id: k.id.to_string(), title: crate::text::printable(&k.title.unwrap_or_default()), key: k.key }
    }
}

/// Bitbucket Cloud's key entries.
#[derive(Deserialize)]
struct BbKey {
    #[serde(default)]
    uuid: String,
    key: String,
    #[serde(default)]
    label: Option<String>,
}

impl From<BbKey> for RegisteredKey {
    fn from(k: BbKey) -> Self {
        RegisteredKey { id: k.uuid, title: crate::text::printable(&k.label.unwrap_or_default()), key: k.key }
    }
}

#[derive(Deserialize)]
struct BbUser {
    uuid: String,
}

/// Where the account's keys are listed and added.
fn keys_url(account: &Account, auth: &Auth) -> Result<String> {
    match account.provider {
        Provider::GitHub | Provider::GitHubEnterprise => Ok(format!("{}/user/keys", super::github::api(&account.host, account.provider))),
        Provider::GitLab | Provider::GitLabSelfManaged => Ok(format!("https://{}/api/v4/user/keys", account.host)),
        Provider::Bitbucket => {
            let me: BbUser = get(auth, &format!("{CLOUD_API}/user"))?;
            Ok(format!("{CLOUD_API}/users/{}/ssh-keys", enc(&me.uuid)))
        }
        _ => Err(unsupported(account)),
    }
}

fn list_at(account: &Account, auth: &Auth, url: &str) -> Result<Vec<RegisteredKey>> {
    if account.provider == Provider::Bitbucket {
        let keys = super::bitbucket::cloud_list::<BbKey>(auth, format!("{url}?pagelen=100"))?;
        return Ok(keys.into_iter().map(Into::into).collect());
    }
    let keys = page::by_link::<HubKey>(auth, format!("{url}?per_page=100"))?;
    Ok(keys.into_iter().map(Into::into).collect())
}

/// The SSH keys on the signed-in user's account.
pub fn list_ssh_keys(account: &Account, auth: &Auth) -> Result<Vec<RegisteredKey>> {
    if !account.provider.adds_ssh_keys() {
        return Err(unsupported(account));
    }
    let url = keys_url(account, auth)?;
    list_at(account, auth, &url)
}

/// Adds `public_key` to the signed-in user's account as `title`, unless a key with the same
/// type and body is there already.
pub fn add_ssh_key(account: &Account, auth: &Auth, public_key: &str, title: &str) -> Result<AddKeyOutcome> {
    if !account.provider.adds_ssh_keys() {
        return Err(unsupported(account));
    }
    let url = keys_url(account, auth)?;
    if let Some(k) = list_at(account, auth, &url)?.into_iter().find(|k| same_key(&k.key, public_key)) {
        return Ok(AddKeyOutcome::AlreadyThere { title: k.title });
    }
    let added: RegisteredKey = if account.provider == Provider::Bitbucket {
        post::<BbKey>(auth, &url, json!({ "key": public_key, "label": title }))?.into()
    } else {
        post::<HubKey>(auth, &url, json!({ "title": title, "key": public_key }))?.into()
    };
    let title = if added.title.is_empty() { title.to_owned() } else { added.title };
    Ok(AddKeyOutcome::Added { title })
}

#[cfg(test)]
mod tests;

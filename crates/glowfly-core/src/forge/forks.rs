//! Forks of a repository on its hosting service: listing them, finding the signed-in user's own
//! fork, and forking into the user's account. Every fork the services send passes one check
//! (`checked`) before Glowfly shows it or adds it as a remote.

use super::remote::{scp_like, split_url};
use super::{Auth, Connection, Target, links, page, parse_remote, service};
use crate::error::{Error, Result};
use crate::remote_ops::{check_url, is_valid_remote_name};
use std::time::Duration;

pub(super) mod bitbucket;
pub(super) mod github;
pub(super) mod gitlab;

/// A fork of a repository, as the service describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fork {
    /// The account it lives in: a login, a namespace path, a workspace or a user slug.
    pub owner: String,
    /// `owner/name` as shown.
    pub full_name: String,
    /// Empty when the service sent none that passed the checks.
    pub https_url: String,
    pub ssh_url: String,
    pub web_url: String,
    pub private: bool,
}

impl Fork {
    /// The clone URL in the same style as `remote_url` (an ssh or scp-like remote gets the SSH
    /// URL, anything else the https one), or the other one when that is missing.
    pub fn url_like(&self, remote_url: &str) -> &str {
        let ssh = remote_url.trim().starts_with("ssh://") || scp_like(remote_url);
        let (first, second) = if ssh { (&self.ssh_url, &self.https_url) } else { (&self.https_url, &self.ssh_url) };
        if first.is_empty() { second } else { first }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ForkList {
    /// Newest first.
    pub forks: Vec<Fork>,
    /// The service had more than Glowfly lists (`page::MAX_ITEMS`).
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForkLookup {
    /// The signed-in user's login.
    pub user: String,
    /// The fork of this repository the user already has.
    pub mine: Option<Fork>,
    /// The repository is in the user's own account, so there is nothing to fork.
    pub own_repository: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreatedFork {
    pub fork: Fork,
    /// The service finished preparing it within the wait, so it can be fetched.
    pub ready: bool,
}

/// How long to wait for a new fork to be ready: up to `attempts` checks, `interval` apart.
pub struct Wait {
    pub attempts: u32,
    pub interval: Duration,
}

impl Default for Wait {
    fn default() -> Self {
        Wait { attempts: 15, interval: Duration::from_secs(2) }
    }
}

impl Wait {
    /// One check, no sleeping (tests).
    pub fn none() -> Wait {
        Wait { attempts: 1, interval: Duration::ZERO }
    }
}

pub fn list_forks(t: &Target, auth: &Auth) -> Result<ForkList> {
    let list = service(t.provider).forks(t, auth)?;
    let truncated = list.truncated || list.forks.len() >= page::MAX_ITEMS;
    Ok(ForkList { forks: list.forks.into_iter().filter_map(|f| checked(t, f)).collect(), truncated })
}

pub fn find_my_fork(t: &Target, auth: &Auth) -> Result<ForkLookup> {
    let mut lookup = service(t.provider).my_fork(t, auth)?;
    lookup.user = crate::text::printable(&lookup.user);
    lookup.mine = lookup.mine.and_then(|f| checked(t, f));
    Ok(lookup)
}

pub fn create_fork(t: &Target, auth: &Auth, wait: &Wait) -> Result<CreatedFork> {
    let created = service(t.provider).create_fork(t, auth, wait)?;
    match checked(t, created.fork) {
        Some(fork) => Ok(CreatedFork { fork, ready: created.ready }),
        None => Err(Error::Other("the service sent no usable clone URL for the fork".into())),
    }
}

/// `url` without a `user@` part after `https://`.
fn without_userinfo(url: &str) -> String {
    let Some(rest) = url.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")).map(|_| &url[8..]) else { return url.to_owned() };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    match rest[..end].rfind('@') {
        Some(at) => format!("{}{}", &url[..8], &rest[at + 1..]),
        None => url.to_owned(),
    }
}

fn hostname(host: &str) -> &str {
    host.split(':').next().unwrap_or(host)
}

/// The fork with only what is safe to show, open and clone: printable names, web links on the
/// service's host, an https clone URL on that host and an SSH one on the same machine. None
/// when neither clone URL is left.
pub(super) fn checked(t: &Target, f: Fork) -> Option<Fork> {
    let host = t.host.to_ascii_lowercase();
    let https = without_userinfo(f.https_url.trim());
    let https_url = if links::https_host(&https).as_deref() == Some(host.as_str()) { https } else { String::new() };
    let ssh = f.ssh_url.trim();
    let clean = !ssh.chars().any(|c| c.is_whitespace() || c.is_control() || crate::text::is_bidi_control(c));
    let ssh_style = ssh.starts_with("ssh://") || scp_like(ssh);
    let same_host = split_url(ssh).is_some_and(|(h, _, is_ssh)| is_ssh && hostname(&h) == hostname(&host));
    let ssh_url = if check_url(ssh).is_ok() && clean && ssh_style && same_host { ssh.to_owned() } else { String::new() };
    if https_url.is_empty() && ssh_url.is_empty() {
        return None;
    }
    Some(Fork {
        owner: crate::text::printable(&f.owner),
        full_name: crate::text::printable(&f.full_name),
        https_url,
        ssh_url,
        web_url: links::web_url(t, f.web_url),
        private: f.private,
    })
}

/// A valid, unused remote name for a fork's owner: characters a remote name can't hold become
/// `-`; an empty result is `fork`; a taken name (in any letter case) gets `-fork`, then `-2`,
/// `-3`…
pub fn remote_name_for(owner: &str, existing: &[&str]) -> String {
    let mut name: String = owner.chars().map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    while name.contains("..") {
        name = name.replace("..", "-");
    }
    let mut name = name.trim_matches(['-', '.']).to_owned();
    if name.to_ascii_lowercase().ends_with(".lock") {
        name.truncate(name.len() - 5);
        name.push_str("-lock");
    }
    if name.is_empty() || !is_valid_remote_name(&name) {
        name = "fork".into();
    }
    let taken = |n: &str| existing.iter().any(|e| e.to_lowercase() == n.to_lowercase());
    if !taken(&name) {
        return name;
    }
    let fork = format!("{name}-fork");
    if !taken(&fork) {
        return fork;
    }
    (2..).map(|i| format!("{name}-{i}")).find(|n| !taken(n)).unwrap_or(fork)
}

/// Whether two remote URLs name the same hosted repository: https and SSH forms, a `.git`
/// suffix, letter case and ports don't matter. Other URLs (folders) are compared as written.
pub fn same_repository(a: &str, b: &str, connections: &[Connection]) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let low = |s: &str| s.to_lowercase();
    if let (Some(x), Some(y)) = (parse_remote(a, connections), parse_remote(b, connections)) {
        return x.provider == y.provider
            && hostname(&low(&x.host)) == hostname(&low(&y.host))
            && low(&x.owner) == low(&y.owner)
            && low(&x.project) == low(&y.project)
            && low(&x.repo) == low(&y.repo);
    }
    match (split_url(a), split_url(b)) {
        (Some((ha, sa, _)), Some((hb, sb, _))) => hostname(&ha) == hostname(&hb) && low(&sa.join("/")) == low(&sb.join("/")),
        _ => a == b,
    }
}

#[cfg(test)]
mod tests;

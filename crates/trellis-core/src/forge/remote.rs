//! Remote detection: which hosting service a remote URL points at.

use super::*;
use serde::{Deserialize, Serialize};

/// A stored connection, from settings (used to recognize self-hosted servers).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub account: Account,
    pub user: String,
    pub method: String,
}

/// SSH addresses that services run on another host name (SSH over port 443), and their web host.
const SSH_ALIASES: [(&str, &str); 3] =
    [("ssh.github.com", "github.com"), ("altssh.gitlab.com", "gitlab.com"), ("altssh.bitbucket.org", "bitbucket.org")];

/// Whether `url` is git's scp-like form, `[user@]host:path`: no scheme and no slash before the
/// first colon. A Windows drive letter (`D:/x`) is a local path, not a host.
pub(super) fn scp_like(url: &str) -> bool {
    let url = url.trim();
    let Some((authority, _)) = url.split_once(':') else { return false };
    let host = authority.rsplit('@').next().unwrap_or_default();
    let drive = authority.len() == 1 && authority.chars().all(|c| c.is_ascii_alphabetic());
    !url.contains("://") && !authority.contains(['/', '\\']) && !host.is_empty() && !drive
}

/// Splits an https, http, ssh or scp-like remote URL into (host, decoded path segments, is
/// ssh). The host is lowercased and keeps an https port; SSH aliases become their web host.
pub(super) fn split_url(url: &str) -> Option<(String, Vec<String>, bool)> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let (host, path, ssh) = if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        let (authority, path) = rest.split_once('/')?;
        (authority.rsplit('@').next()?.to_owned(), path.to_owned(), false)
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        (host.split(':').next()?.to_owned(), path.to_owned(), true)
    } else if scp_like(url) {
        let (authority, path) = url.split_once(':')?;
        (authority.rsplit('@').next()?.to_owned(), path.to_owned(), true)
    } else {
        return None;
    };
    let segs = path.split('/').filter(|s| !s.is_empty()).map(dec).collect();
    let host = host.to_ascii_lowercase();
    let host = SSH_ALIASES.iter().find(|(alias, _)| *alias == host).map_or(host, |(_, web)| (*web).to_owned());
    Some((host, segs, ssh))
}

/// Recognizes the hosting service behind a remote URL. Self-hosted servers are matched against
/// the hosts of saved connections, then by common URL shapes.
pub fn parse_remote(url: &str, connections: &[Connection]) -> Option<Target> {
    if let Some(t) = azure::parse_remote(url) {
        return Some(t);
    }
    let (host, segs, _ssh) = split_url(url)?;
    let owner_repo = |segs: &[String]| -> Option<(String, String)> {
        let (repo, owner) = segs.split_last()?;
        (!owner.is_empty()).then(|| (owner.join("/"), repo.clone()))
    };
    let make = |provider: Provider, host: &str, segs: &[String]| {
        let (owner, repo) = owner_repo(segs)?;
        Some(Target { provider, host: host.to_owned(), owner, project: String::new(), repo })
    };
    match host.as_str() {
        "github.com" => return make(Provider::GitHub, &host, &segs[..segs.len().min(2)]),
        "gitlab.com" => return make(Provider::GitLab, &host, &segs),
        "bitbucket.org" => return make(Provider::Bitbucket, &host, &segs[..segs.len().min(2)]),
        _ => {}
    }
    let bitbucket_dc = |segs: &[String]| {
        // https://host/scm/PROJ/repo or ssh://git@host:7999/PROJ/repo
        let segs = if segs.first().is_some_and(|s| s == "scm") { &segs[1..] } else { segs };
        let (owner, repo) = owner_repo(segs)?;
        Some(Target { provider: Provider::BitbucketDataCenter, host: host.clone(), owner, project: String::new(), repo })
    };
    if let Some(c) = connections.iter().find(|c| c.account.host == host && !c.account.provider.is_jira()) {
        return match c.account.provider {
            Provider::BitbucketDataCenter => bitbucket_dc(&segs),
            p => make(p, &host, &segs),
        };
    }
    if segs.first().is_some_and(|s| s == "scm") {
        return bitbucket_dc(&segs);
    }
    if host.contains("gitlab") {
        return make(Provider::GitLabSelfManaged, &host, &segs);
    }
    if host.starts_with("github.") {
        return make(Provider::GitHubEnterprise, &host, &segs);
    }
    if host.contains("bitbucket") {
        return bitbucket_dc(&segs);
    }
    None
}

/// Parses what the user typed into a "Host Domain" field for a provider.
/// Returns (host, organization) — the organization only applies to Azure DevOps.
pub fn parse_host(provider: Provider, text: &str) -> Option<(String, String)> {
    if let Some(h) = provider.fixed_host() {
        return Some((h.to_owned(), String::new()));
    }
    if provider == Provider::AzureDevOps {
        return azure::parse_host_domain(text);
    }
    let t = text.trim();
    let t = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(t);
    let host = t.split('/').next()?.rsplit('@').next()?.to_ascii_lowercase();
    (host.contains('.') || host.contains(':') || host == "localhost").then_some((host, String::new()))
}

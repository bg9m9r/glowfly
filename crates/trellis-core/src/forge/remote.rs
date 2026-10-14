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

/// Splits an https or ssh remote URL into (host, path segments).
fn split_url(url: &str) -> Option<(String, Vec<String>, bool)> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let (host, path, ssh) = if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        let (authority, path) = rest.split_once('/')?;
        (authority.rsplit('@').next()?.to_owned(), path.to_owned(), false)
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        (host.split(':').next()?.to_owned(), path.to_owned(), true)
    } else if let Some((authority, path)) = url.split_once(':') {
        // scp-like: git@host:owner/repo
        if authority.contains('/') {
            return None;
        }
        (authority.rsplit('@').next()?.to_owned(), path.to_owned(), true)
    } else {
        return None;
    };
    let segs = path.split('/').filter(|s| !s.is_empty()).map(dec).collect();
    Some((host.to_ascii_lowercase(), segs, ssh))
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

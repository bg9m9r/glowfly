//! Links that came back from a hosting service's API. A compromised or malicious server could
//! return any URL, so only `https://` links on the service's own host are kept (build pages,
//! which often live on another site, only need to be safe `https://` links).

use super::{Checks, Issue, PrDetails, Provider, PullRequest, Target};
use std::collections::HashMap;

/// The host part of an `https://` URL, lowercased, when the URL is safe to hand to a browser:
/// no whitespace, control or bidi characters, no `user@` part and no backslashes.
pub fn https_host(url: &str) -> Option<String> {
    if url.chars().any(|c| c.is_whitespace() || c.is_control() || crate::text::is_bidi_control(c) || c == '\\') {
        return None;
    }
    let rest = url.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")).map(|_| &url[8..])?;
    let authority = rest.split(['/', '?', '#']).next()?;
    (!authority.is_empty() && !authority.contains('@')).then(|| authority.to_ascii_lowercase())
}

/// `url` when it is an https link on one of the hosts this service serves web pages from,
/// otherwise an empty string (the link is not offered).
pub(super) fn web_url(t: &Target, url: String) -> String {
    let Some(host) = https_host(&url) else { return String::new() };
    // Azure DevOps answers for *.visualstudio.com organizations on dev.azure.com too.
    let ok = host == t.host.to_ascii_lowercase() || (t.provider == Provider::AzureDevOps && host == "dev.azure.com");
    if ok { url } else { String::new() }
}

/// `url` when it is a safe `https://` link on any host (CI often runs on another site),
/// otherwise an empty string.
pub(super) fn build_url(url: String) -> String {
    if https_host(&url).is_some() { url } else { String::new() }
}

pub(super) fn checks_map(mut map: HashMap<String, Checks>) -> HashMap<String, Checks> {
    for c in map.values_mut() {
        c.url = build_url(std::mem::take(&mut c.url));
    }
    map
}

pub(super) fn prs(t: &Target, prs: Vec<PullRequest>) -> Vec<PullRequest> {
    prs.into_iter().map(|p| pr(t, p)).collect()
}

pub(super) fn pr(t: &Target, mut p: PullRequest) -> PullRequest {
    p.url = web_url(t, std::mem::take(&mut p.url));
    if let Some(c) = &mut p.checks {
        c.url = build_url(std::mem::take(&mut c.url));
    }
    p
}

pub(super) fn details(mut v: Vec<PrDetails>) -> Vec<PrDetails> {
    for c in v.iter_mut().filter_map(|d| d.checks.as_mut()) {
        c.url = build_url(std::mem::take(&mut c.url));
    }
    v
}

pub(super) fn issues(t: &Target, items: Vec<Issue>) -> Vec<Issue> {
    items.into_iter().map(|i| issue(t, i)).collect()
}

pub(super) fn issue(t: &Target, mut i: Issue) -> Issue {
    i.url = web_url(t, std::mem::take(&mut i.url));
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(provider: Provider, host: &str) -> Target {
        Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
    }

    #[test]
    fn keeps_only_https_links_on_the_service_host() {
        let gh = t(Provider::GitHub, "github.com");
        let keep = |u: &str| web_url(&gh, u.to_owned());
        assert_eq!(keep("https://github.com/o/r/pull/4"), "https://github.com/o/r/pull/4");
        assert_eq!(keep("HTTPS://GitHub.com/o/r/pull/4"), "HTTPS://GitHub.com/o/r/pull/4");
        for bad in [
            "http://github.com/o/r/pull/4",
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "https://github.com.evil.example/o",
            "https://evil.example/github.com",
            "https://github.com@evil.example/o",
            r"https://github.com\@evil.example",
            "https://github.com/o r",
            "https://github.com/\u{202e}fdp.exe",
            r"\\server\share\x.exe",
            "",
        ] {
            assert_eq!(keep(bad), "", "{bad}");
        }
        let ghe = t(Provider::GitHubEnterprise, "git.acme.io:8443");
        assert_eq!(web_url(&ghe, "https://git.acme.io:8443/o/r/pull/1".into()), "https://git.acme.io:8443/o/r/pull/1");
        assert_eq!(web_url(&ghe, "https://git.acme.io/o/r/pull/1".into()), "");
        let az = t(Provider::AzureDevOps, "contoso.visualstudio.com");
        assert_eq!(
            web_url(&az, "https://dev.azure.com/contoso/p/_git/r/pullrequest/1".into()),
            "https://dev.azure.com/contoso/p/_git/r/pullrequest/1"
        );
    }

    #[test]
    fn build_url_keeps_https_links_on_any_host() {
        assert_eq!(build_url("https://ci.example.com/build/7".into()), "https://ci.example.com/build/7");
        for bad in [
            "http://ci.example.com/build/7",
            "javascript:alert(1)",
            "https://a@b/x",
            r"https://ci.example.com\x",
            "https://ci.example.com/\u{202e}x",
        ] {
            assert_eq!(build_url(bad.into()), "", "{bad}");
        }
        let pr_with = |url: &str| PullRequest {
            id: 1,
            title: String::new(),
            description: String::new(),
            source: String::new(),
            target: String::new(),
            author: String::new(),
            draft: false,
            reviewers: Vec::new(),
            head: None,
            head_repo: Default::default(),
            merge_status: None,
            checks: Some(Checks { url: url.into(), ..Checks::from_counts(1, 0, 0, true) }),
            url: String::new(),
            facets: Default::default(),
        };
        let gh = t(Provider::GitHub, "github.com");
        assert_eq!(pr(&gh, pr_with("http://ci.example.com/1")).checks.unwrap().url, "");
        assert_eq!(pr(&gh, pr_with("https://ci.example.com/1")).checks.unwrap().url, "https://ci.example.com/1");
        let map =
            checks_map(HashMap::from([("a".to_owned(), Checks { url: "javascript:x".into(), ..Checks::from_counts(1, 0, 0, true) })]));
        assert_eq!(map["a"].url, "");
    }
}

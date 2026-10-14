//! Links that came back from a hosting service's API. A compromised or malicious server could
//! return any URL, so only `https://` links on the service's own host are kept.

use super::{Issue, Provider, PullRequest, Target};

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

pub(super) fn prs(t: &Target, prs: Vec<PullRequest>) -> Vec<PullRequest> {
    prs.into_iter().map(|p| pr(t, p)).collect()
}

pub(super) fn pr(t: &Target, mut p: PullRequest) -> PullRequest {
    p.url = web_url(t, std::mem::take(&mut p.url));
    p
}

pub(super) fn issues(t: &Target, items: Vec<Issue>) -> Vec<Issue> {
    items
        .into_iter()
        .map(|mut i| {
            i.url = web_url(t, std::mem::take(&mut i.url));
            i
        })
        .collect()
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
}

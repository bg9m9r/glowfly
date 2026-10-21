//! Repository web pages: which page in a browser belongs to a remote URL's repository. Pure (no
//! I/O), so views may call it to label and enable menu items.

use super::remote::{scp_like, split_url};
use super::{Connection, enc, links, parse_remote};

/// A repository's home page on its hosting service.
#[derive(Clone, Debug, PartialEq)]
pub struct WebPage {
    /// "GitHub", "Codeberg"…; empty for an https host Trellis doesn't know ("in browser").
    pub service: String,
    pub url: String,
}

/// Why a remote has no web page to open.
#[derive(Clone, Debug, PartialEq)]
pub enum NoWebPage {
    NoUrl,
    /// A folder path or a file:// URL.
    Local,
    /// Plain http on a host Trellis doesn't know.
    PlainHttp,
    /// An SSH address on this host, which Trellis doesn't know.
    UnknownSsh(String),
    /// The URL fails the link safety check.
    Unsafe,
}

impl NoWebPage {
    pub fn message(&self) -> String {
        match self {
            NoWebPage::NoUrl => "This remote has no fetch URL.".into(),
            NoWebPage::Local => "This remote is a folder on disk, not a web page.".into(),
            NoWebPage::PlainHttp => "This remote uses plain http; Trellis opens only https pages.".into(),
            NoWebPage::UnknownSsh(host) => format!("Trellis can't tell which web page belongs to the SSH address on {host}."),
            NoWebPage::Unsafe => "This remote's URL can't be opened safely in a browser.".into(),
        }
    }
}

/// Code hosts without an API integration whose repository pages follow the remote's path.
const EXTRA_HOSTS: [(&str, &str); 3] = [("codeberg.org", "Codeberg"), ("gitea.com", "Gitea"), ("git.sr.ht", "SourceHut")];

fn is_local(url: &str) -> bool {
    url.get(..7).is_some_and(|s| s.eq_ignore_ascii_case("file://")) || (!url.contains("://") && !scp_like(url))
}

/// The web page of the repository a remote URL names, or why there is none.
pub fn web_page(remote_url: &str, connections: &[Connection]) -> Result<WebPage, NoWebPage> {
    let url = remote_url.trim();
    if url.is_empty() {
        return Err(NoWebPage::NoUrl);
    }
    if is_local(url) {
        return Err(NoWebPage::Local);
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control() || crate::text::is_bidi_control(c) || c == '\\') {
        return Err(NoWebPage::Unsafe);
    }
    let page = |service: &str, url: String| match links::https_host(&url) {
        Some(_) => Ok(WebPage { service: service.to_owned(), url }),
        None => Err(NoWebPage::Unsafe),
    };
    if let Some(t) = parse_remote(url, connections) {
        let checked = links::web_url(&t, t.provider.info().service.repo_url(&t));
        return if checked.is_empty() { Err(NoWebPage::Unsafe) } else { page(t.provider.name(), checked) };
    }
    let Some((host, segs, ssh)) = split_url(url) else { return Err(NoWebPage::Unsafe) };
    if let Some((_, service)) = EXTRA_HOSTS.iter().find(|(h, _)| *h == host) {
        let path: Vec<String> = segs.iter().map(|s| enc(s)).collect();
        return page(service, format!("https://{host}/{}", path.join("/")));
    }
    if let Some(rest) = url.strip_prefix("https://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let authority = authority.rsplit('@').next().unwrap_or_default();
        let path = path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).trim_end_matches('/');
        return page("", format!("https://{authority}/{path}"));
    }
    if url.starts_with("http://") {
        return Err(NoWebPage::PlainHttp);
    }
    if ssh { Err(NoWebPage::UnknownSsh(host)) } else { Err(NoWebPage::Unsafe) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::{Account, Provider};

    fn conn(provider: Provider, host: &str) -> Connection {
        Connection { account: Account { provider, host: host.into(), org: String::new() }, user: "u".into(), method: "m".into() }
    }

    fn ok(url: &str, connections: &[Connection]) -> (String, String) {
        let p = web_page(url, connections).unwrap_or_else(|e| panic!("{url}: {e:?}"));
        (p.service, p.url)
    }

    fn s(service: &str, url: &str) -> (String, String) {
        (service.to_owned(), url.to_owned())
    }

    #[test]
    fn code_hosts_open_the_repository_home_page() {
        for url in ["https://github.com/o/r.git", "git@github.com:o/r.git", "ssh://git@github.com/o/r", "http://github.com/o/r"] {
            assert_eq!(ok(url, &[]), s("GitHub", "https://github.com/o/r"), "{url}");
        }
        assert_eq!(ok("ssh://git@ssh.github.com:443/o/r.git", &[]), s("GitHub", "https://github.com/o/r"));
        assert_eq!(ok("git@gitlab.com:g/sub/app.git", &[]), s("GitLab", "https://gitlab.com/g/sub/app"));
        assert_eq!(ok("git@bitbucket.org:team/app.git", &[]), s("Bitbucket", "https://bitbucket.org/team/app"));
        let dc = [conn(Provider::BitbucketDataCenter, "bb.acme.io")];
        let dc_name = "Bitbucket Data Center";
        assert_eq!(ok("https://bb.acme.io/scm/PROJ/app.git", &dc), s(dc_name, "https://bb.acme.io/projects/PROJ/repos/app/browse"));
        assert_eq!(ok("https://bb.acme.io/scm/~ada/app.git", &dc), s(dc_name, "https://bb.acme.io/users/ada/repos/app/browse"));
        let az = s("Azure DevOps", "https://dev.azure.com/org/proj/_git/app");
        assert_eq!(ok("https://dev.azure.com/org/proj/_git/app", &[]), az);
        assert_eq!(ok("git@ssh.dev.azure.com:v3/org/proj/app", &[]), az);
        assert_eq!(
            ok("https://contoso.visualstudio.com/proj/_git/app", &[]),
            s("Azure DevOps", "https://contoso.visualstudio.com/proj/_git/app")
        );
        let ghe = [conn(Provider::GitHubEnterprise, "git.acme.io:8443")];
        assert_eq!(ok("https://git.acme.io:8443/o/r.git", &ghe), s("GitHub Enterprise Server", "https://git.acme.io:8443/o/r"));
        // Escapes stay escaped: they never become path steps.
        assert_eq!(ok("https://github.com/%2E%2E/r", &[]), s("GitHub", "https://github.com/%2E%2E/r"));
    }

    #[test]
    fn other_hosts() {
        assert_eq!(ok("git@codeberg.org:ada/app.git", &[]), s("Codeberg", "https://codeberg.org/ada/app"));
        assert_eq!(ok("https://gitea.com/ada/app", &[]), s("Gitea", "https://gitea.com/ada/app"));
        assert_eq!(ok("git@git.sr.ht:~ada/app", &[]), s("SourceHut", "https://git.sr.ht/~ada/app"));
        assert_eq!(ok("https://dev:pw@example.com/team/app.git", &[]), s("", "https://example.com/team/app"));
        assert_eq!(ok("https://example.com/team/app/", &[]), s("", "https://example.com/team/app"));
    }

    #[test]
    fn remotes_without_a_page() {
        assert_eq!(web_page("http://example.com/x", &[]), Err(NoWebPage::PlainHttp));
        assert_eq!(web_page("git@example.com:x/y", &[]), Err(NoWebPage::UnknownSsh("example.com".into())));
        assert_eq!(web_page("ssh://git@example.com:22/x/y", &[]), Err(NoWebPage::UnknownSsh("example.com".into())));
        for local in ["D:/backups/app.git", r"D:\backups\app.git", "/srv/git/app", "../app", "file:///srv/app", "FILE:///srv/app"] {
            assert_eq!(web_page(local, &[]), Err(NoWebPage::Local), "{local}");
        }
        assert_eq!(web_page("  ", &[]), Err(NoWebPage::NoUrl));
        for bad in [
            "https://github.com/o/r\u{202e}x",
            "https://example.com/a b",
            "https://example.com/a\u{1b}b",
            r"https://example.com\@evil.example/x",
        ] {
            assert_eq!(web_page(bad, &[]), Err(NoWebPage::Unsafe), "{bad}");
        }
        let all = [NoWebPage::NoUrl, NoWebPage::Local, NoWebPage::PlainHttp, NoWebPage::UnknownSsh("h".into()), NoWebPage::Unsafe];
        let texts: std::collections::HashSet<String> = all.iter().map(NoWebPage::message).collect();
        assert_eq!(texts.len(), all.len());
        assert!(texts.iter().all(|t| !t.is_empty()));
    }
}

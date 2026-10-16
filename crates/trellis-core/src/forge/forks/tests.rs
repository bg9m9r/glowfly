use super::*;
use crate::forge::Provider;

fn gh() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "team".into(), project: String::new(), repo: "app".into() }
}

fn fork(https: &str, ssh: &str) -> Fork {
    Fork {
        owner: "ada".into(),
        full_name: "ada/app".into(),
        https_url: https.into(),
        ssh_url: ssh.into(),
        web_url: "https://github.com/ada/app".into(),
        private: false,
    }
}

#[test]
fn remote_names_for_fork_owners() {
    assert_eq!(remote_name_for("ada", &[]), "ada");
    assert_eq!(remote_name_for("Group/Sub", &[]), "Group-Sub");
    for owner in ["~ADA", "bad name..lock", "a@{b}", ".hidden", "x.lock", "a\u{1b}b"] {
        let name = remote_name_for(owner, &[]);
        assert!(is_valid_remote_name(&name), "{owner} → {name}");
    }
    assert_eq!(remote_name_for("~ADA", &[]), "ADA");
    assert_eq!(remote_name_for("", &[]), "fork");
    assert_eq!(remote_name_for("~/.", &[]), "fork");
    assert_eq!(remote_name_for("ada", &["origin", "ada"]), "ada-fork");
    assert_eq!(remote_name_for("ada", &["ada", "ada-fork"]), "ada-2");
    assert_eq!(remote_name_for("ada", &["ada", "ada-fork", "ada-2"]), "ada-3");
    assert_eq!(remote_name_for("Ada", &["ada"]), "Ada-fork", "taken in another letter case");
}

#[test]
fn same_repository_ignores_url_style() {
    let same = |a: &str, b: &str| same_repository(a, b, &[]);
    assert!(same("https://github.com/ada/app.git", "git@github.com:ada/app.git"));
    assert!(same("ssh://git@github.com/Ada/App", "https://github.com/ada/app"));
    assert!(same("https://github.com/ada/app/", "ssh://git@ssh.github.com:443/ada/app.git"));
    assert!(same("https://example.com:8443/team/app.git", "ssh://git@example.com:22/team/app"));
    assert!(!same("https://github.com/ada/app", "https://github.com/ada/app2"));
    assert!(!same("https://github.com/ada/app", "https://github.com/grace/app"));
    assert!(same("D:/backups/app.git", "D:/backups/app.git"));
    assert!(!same("D:/backups/app.git", "D:/backups/app"));
    assert!(!same("", ""));
}

#[test]
fn url_like_follows_the_remote() {
    let f = fork("https://github.com/ada/app.git", "git@github.com:ada/app.git");
    assert_eq!(f.url_like("git@github.com:team/app.git"), "git@github.com:ada/app.git");
    assert_eq!(f.url_like("ssh://git@github.com/team/app"), "git@github.com:ada/app.git");
    assert_eq!(f.url_like("https://github.com/team/app.git"), "https://github.com/ada/app.git");
    assert_eq!(fork("", "git@github.com:ada/app.git").url_like("https://github.com/team/app"), "git@github.com:ada/app.git");
    assert_eq!(fork("https://github.com/ada/app.git", "").url_like("git@github.com:team/app"), "https://github.com/ada/app.git");
}

#[test]
fn checked_keeps_only_safe_links() {
    let t = gh();
    let ok = checked(&t, fork("https://github.com/ada/app.git", "git@github.com:ada/app.git")).unwrap();
    assert_eq!((ok.https_url.as_str(), ok.ssh_url.as_str()), ("https://github.com/ada/app.git", "git@github.com:ada/app.git"));
    let mut evil = fork("https://evil.example/ada/app.git", "git@evil.example:ada/app.git");
    evil.web_url = "https://github.com.evil.example/ada/app".into();
    evil.https_url = "https://github.com/ada/app.git".into();
    let c = checked(&t, evil).unwrap();
    assert_eq!((c.web_url.as_str(), c.ssh_url.as_str()), ("", ""), "off-host links are dropped");
    for bad_ssh in ["-oProxyCommand=x@github.com:a/b", "git@github.com:a/b c", "ext::sh -c x", "git@github.com:a/\u{202e}b", "/srv/x"] {
        assert_eq!(checked(&t, fork("https://github.com/ada/app.git", bad_ssh)).unwrap().ssh_url, "", "{bad_ssh}");
    }
    assert_eq!(checked(&t, fork("http://github.com/ada/app.git", "git@github.com:ada/app.git")).unwrap().https_url, "");
    let stripped = checked(&t, fork("https://ada:token@github.com/ada/app.git", "")).unwrap();
    assert_eq!(stripped.https_url, "https://github.com/ada/app.git", "user@ stripped");
    assert_eq!(checked(&t, fork("https://evil.example/x.git", "-x")), None, "no usable URL");
    let mut sneaky = fork("https://github.com/ada/app.git", "");
    sneaky.owner = "ad\u{1b}[2Ja\u{202e}".into();
    sneaky.full_name = "ad\u{7}a/app".into();
    let c = checked(&t, sneaky).unwrap();
    assert_eq!((c.owner.as_str(), c.full_name.as_str()), ("ad[2Ja", "ada/app"));
    // An SSH host with a port matches the service host without one.
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bb.acme.io".into(), ..gh() };
    let c = checked(&dc, fork("https://bb.acme.io/scm/~ada/app.git", "ssh://git@bb.acme.io:7999/~ada/app.git")).unwrap();
    assert_eq!(c.ssh_url, "ssh://git@bb.acme.io:7999/~ada/app.git");
}

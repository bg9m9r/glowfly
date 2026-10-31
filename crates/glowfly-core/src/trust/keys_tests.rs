//! Which settings `command_key` lists: git-lfs's programs, and TLS and proxy settings whatever
//! their value.

use super::super::{ConfigCommand, scan_at};
use super::*;
use crate::testutil::*;

fn listed(name: &str, value: &str) -> Option<String> {
    command_key(name, value)
}

#[test]
fn git_lfs_programs_are_listed() {
    assert_eq!(listed("lfs.customtransfer.agent.path", "/opt/agent").as_deref(), Some("lfs.customtransfer.agent.path"));
    assert_eq!(listed("lfs.customtransfer.Agent.args", "--fast").as_deref(), Some("lfs.customtransfer.Agent.args"));
    // The subsection keeps the case it was written in; git-lfs may read any.
    assert_eq!(listed("lfs.customTransfer.agent.path", "x").as_deref(), Some("lfs.customTransfer.agent.path"));
    assert_eq!(listed("lfs.standalonetransferagent", "agent").as_deref(), Some("lfs.standaloneTransferAgent"));
    assert_eq!(listed("lfs.extension.foo.clean", "foo clean %f").as_deref(), Some("lfs.extension.foo.clean"));
    assert_eq!(listed("lfs.extension.foo.smudge", "foo smudge %f").as_deref(), Some("lfs.extension.foo.smudge"));
    // What runs nothing: an agent's options, an extension's order, git-lfs's own settings.
    assert_eq!(listed("lfs.customtransfer.agent.concurrent", "true"), None);
    assert_eq!(listed("lfs.extension.foo.priority", "0"), None);
    assert_eq!(listed("lfs.url", "https://example.com/lfs"), None);
    assert_eq!(listed("lfs.fetchinclude", "docs"), None);
    // An empty value turns the setting off.
    assert_eq!(listed("lfs.customtransfer.agent.path", ""), None);
}

#[test]
fn tls_and_proxy_settings_are_listed_whatever_their_value() {
    let cases = [
        ("http.sslverify", "false", "http.sslVerify"),
        // Even the safe value: the repository shouldn't decide it.
        ("http.sslverify", "true", "http.sslVerify"),
        // `sslVerify =` is false to git.
        ("http.sslverify", "", "http.sslVerify"),
        ("http.sslcainfo", "/tmp/ca.pem", "http.sslCAInfo"),
        ("http.sslcapath", "/tmp/certs", "http.sslCAPath"),
        ("http.proxy", "http://proxy.example:8080", "http.proxy"),
        // A host alone is a proxy too.
        ("http.proxy", "proxy.example", "http.proxy"),
        ("http.https://example.com/.sslverify", "false", "http.https://example.com/.sslVerify"),
        ("http.https://example.com/.proxy", "socks5://p", "http.https://example.com/.proxy"),
        ("http.https://a.b.example/x.y/.sslcainfo", "ca.pem", "http.https://a.b.example/x.y/.sslCAInfo"),
        ("remote.origin.proxy", "proxy.example", "remote.origin.proxy"),
    ];
    for (name, value, shown) in cases {
        assert_eq!(listed(name, value).as_deref(), Some(shown), "{name} = {value}");
        assert!(is_network_setting(shown), "{shown}");
    }
    // Other connection settings, and a proxy key elsewhere, aren't.
    for name in ["http.postbuffer", "http.https://example.com/.useragent", "remote.origin.fetch", "core.proxy", "http.sslverifyx"] {
        assert_eq!(listed(name, "x"), None, "{name}");
    }
    assert!(!is_network_setting("core.sshCommand"));
    assert!(!is_network_setting("lfs.customtransfer.x.path"));
    assert!(is_network_setting("HTTP.SSLVERIFY"), "any letter case of the display key");
}

#[test]
fn a_repositorys_own_tls_proxy_and_lfs_settings_are_scanned() {
    let dir = scratch_repo("trust-keys-scan");
    let config = dir.join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(
        "[http]\n\tsslVerify\n\tproxy = proxy.example\n[http \"https://example.com/\"]\n\tsslVerify = false\n\
         [remote \"origin\"]\n\tproxy = socks5://p\n\
         [lfs \"customtransfer.agent\"]\n\tpath = ./bin/agent\n\tconcurrent = true\n[lfs]\n\tstandalonetransferagent = agent\n\
         [lfs \"extension.foo\"]\n\tclean = foo %f\n\tpriority = 0\n",
    );
    std::fs::write(&config, text).unwrap();
    let s = scan_at(&dir).unwrap();
    let got: Vec<(&str, &str)> = s.config.iter().map(|c| (c.key.as_str(), c.value.as_str())).collect();
    assert_eq!(
        got,
        [
            // A bare key means true; it still counts.
            ("http.sslVerify", ""),
            ("http.proxy", "proxy.example"),
            ("http.https://example.com/.sslVerify", "false"),
            ("remote.origin.proxy", "socks5://p"),
            ("lfs.customtransfer.agent.path", "./bin/agent"),
            ("lfs.standaloneTransferAgent", "agent"),
            ("lfs.extension.foo.clean", "foo %f"),
        ]
    );
    assert!(!s.is_empty());
    // A changed value asks again.
    let before = s.print();
    git(&dir, &["config", "http.proxy", "other.example"]);
    assert!(!before.covers(&scan_at(&dir).unwrap().print()));
    assert!(s.config.contains(&ConfigCommand { key: "remote.origin.proxy".into(), value: "socks5://p".into() }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_bytes_that_arent_utf8_read_as_distinct_text() {
    assert_eq!(text_of(b"plain"), "plain");
    assert_eq!(text_of(b"sh \xff.sh"), "sh \u{FFFD}FF.sh");
    // A U+FFFD written in the text is doubled, so no other bytes read the same.
    assert_eq!(text_of("sh \u{FFFD}FF.sh".as_bytes()), "sh \u{FFFD}\u{FFFD}FF.sh");
    assert_ne!(text_of(b"\xfe"), text_of(b"\xff"));
}

#[test]
fn program_settings_that_arent_utf8_are_still_scanned() {
    let dir = scratch_repo("trust-keys-bytes");
    let config = dir.join(".git/config");
    let mut bytes = std::fs::read(&config).unwrap();
    bytes.extend_from_slice(b"[filter \"x\"]\n\tclean = sh \xff.sh\n[filter \"\xfe\"]\n\tsmudge = cat\n");
    std::fs::write(&config, bytes).unwrap();
    let s = scan_at(&dir).unwrap();
    let got: Vec<(&str, &str)> = s.config.iter().map(|c| (c.key.as_str(), c.value.as_str())).collect();
    assert_eq!(got, [("filter.x.clean", "sh \u{FFFD}FF.sh"), ("filter.\u{FFFD}FE.smudge", "cat")]);
    // Another byte in its place asks again.
    let before = s.print();
    let mut bytes = std::fs::read(&config).unwrap();
    let at = bytes.iter().position(|b| *b == 0xff).unwrap();
    bytes[at] = 0xfd;
    std::fs::write(&config, bytes).unwrap();
    assert!(!before.covers(&scan_at(&dir).unwrap().print()));
    let _ = std::fs::remove_dir_all(&dir);
}

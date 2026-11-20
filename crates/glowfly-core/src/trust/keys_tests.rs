//! Which settings `command_key` lists: git-lfs's programs, and TLS, header, redirect, proxy and
//! cookie-file settings whatever their value.

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
        // The file git reads cookies from and, with saveCookies, writes them to.
        ("http.cookiefile", "~/cookies.txt", "http.cookieFile"),
        ("http.savecookies", "true", "http.saveCookies"),
        ("http.savecookies", "false", "http.saveCookies"),
        ("http.https://example.com/.cookiefile", "jar", "http.https://example.com/.cookieFile"),
        ("http.https://example.com/.savecookies", "true", "http.https://example.com/.saveCookies"),
        // The client certificate and key, the TLS library, its versions, ciphers and pinned
        // key, and schannel's revocation check.
        ("http.sslcert", "certs/me.pem", "http.sslCert"),
        ("http.sslkey", "certs/me.key", "http.sslKey"),
        ("http.sslbackend", "openssl", "http.sslBackend"),
        ("http.sslversion", "tlsv1.0", "http.sslVersion"),
        ("http.sslcipherlist", "NULL", "http.sslCipherList"),
        ("http.pinnedpubkey", "sha256//x", "http.pinnedPubkey"),
        ("http.schannelcheckrevoke", "false", "http.schannelCheckRevoke"),
        // Where a host resolves, which redirects git follows, and headers sent with every request.
        ("http.curloptresolve", "example.com:443:203.0.113.5", "http.curloptResolve"),
        ("http.followredirects", "true", "http.followRedirects"),
        ("http.extraheader", "X-Extra: 1", "http.extraHeader"),
        // An empty extraHeader resets the list; it is still the repository's choice.
        ("http.extraheader", "", "http.extraHeader"),
        ("http.https://example.com/.sslcert", "me.pem", "http.https://example.com/.sslCert"),
        ("http.https://example.com/.extraheader", "X-Extra: 1", "http.https://example.com/.extraHeader"),
        ("http.https://example.com/.curloptresolve", "example.com:443:203.0.113.5", "http.https://example.com/.curloptResolve"),
        ("http.https://example.com/.followredirects", "false", "http.https://example.com/.followRedirects"),
        // How git signs in, the certificate's and key's form, and the HTTP version.
        ("http.emptyauth", "true", "http.emptyAuth"),
        ("http.proactiveauth", "basic", "http.proactiveAuth"),
        ("http.delegation", "always", "http.delegation"),
        ("http.sslcertpasswordprotected", "true", "http.sslCertPasswordProtected"),
        ("http.sslcerttype", "P12", "http.sslCertType"),
        ("http.sslkeytype", "ENG", "http.sslKeyType"),
        ("http.version", "HTTP/1.1", "http.version"),
        ("http.https://example.com/.emptyauth", "true", "http.https://example.com/.emptyAuth"),
        ("http.https://example.com/.version", "HTTP/2", "http.https://example.com/.version"),
        // schannel's use of the CA bundle, and the proxy's sign-in, certificate, key and CA bundle.
        ("http.schannelusesslcainfo", "true", "http.schannelUseSSLCAInfo"),
        ("http.proxyauthmethod", "basic", "http.proxyAuthMethod"),
        ("http.proxysslcert", "certs/proxy.pem", "http.proxySSLCert"),
        ("http.proxysslkey", "certs/proxy.key", "http.proxySSLKey"),
        ("http.proxysslcainfo", "certs/proxy-ca.pem", "http.proxySSLCAInfo"),
        ("http.proxysslcertpasswordprotected", "true", "http.proxySSLCertPasswordProtected"),
        ("http.https://example.com/.proxyauthmethod", "anyauth", "http.https://example.com/.proxyAuthMethod"),
        ("http.https://example.com/.proxysslcert", "p.pem", "http.https://example.com/.proxySSLCert"),
        ("http.https://example.com/.schannelusesslcainfo", "false", "http.https://example.com/.schannelUseSSLCAInfo"),
        ("remote.origin.proxyauthmethod", "ntlm", "remote.origin.proxyAuthMethod"),
    ];
    for (name, value, shown) in cases {
        assert_eq!(listed(name, value).as_deref(), Some(shown), "{name} = {value}");
        assert!(is_network_setting(shown), "{shown}");
    }
    // Other connection settings, and a proxy key elsewhere, aren't.
    for name in [
        "http.postbuffer",
        "http.https://example.com/.useragent",
        "remote.origin.fetch",
        "core.proxy",
        "http.sslverifyx",
        "http.sslcertx",
        "remote.proxyauthmethod",
    ] {
        assert_eq!(listed(name, "x"), None, "{name}");
    }
    assert!(!is_network_setting("core.sshCommand"));
    assert!(!is_network_setting("lfs.customtransfer.x.path"));
    assert!(is_network_setting("HTTP.SSLVERIFY"), "any letter case of the display key");
    assert!(is_network_setting("http.https://Example.com/.ExtraHeader"));
}

#[test]
fn a_repositorys_own_tls_proxy_and_lfs_settings_are_scanned() {
    let dir = scratch_repo("trust-keys-scan");
    let config = dir.join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(
        "[http]\n\tsslVerify\n\tproxy = proxy.example\n\tcookieFile = ../jar\n\tsaveCookies\n\
         [http \"https://example.com/\"]\n\tsslVerify = false\n\textraHeader = X-Extra: 1\n\tcurloptResolve = example.com:443:203.0.113.5\n\
         [http]\n\tsslCert = certs/me.pem\n\tfollowRedirects = true\n\tpostBuffer = 1048576\n\
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
            ("http.cookieFile", "../jar"),
            ("http.saveCookies", ""),
            ("http.https://example.com/.sslVerify", "false"),
            ("http.https://example.com/.extraHeader", "X-Extra: 1"),
            ("http.https://example.com/.curloptResolve", "example.com:443:203.0.113.5"),
            ("http.sslCert", "certs/me.pem"),
            ("http.followRedirects", "true"),
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
fn a_changed_certificate_key_or_pinned_key_file_asks_again() {
    let dir = scratch_repo("trust-keys-tls-files");
    std::fs::create_dir_all(dir.join("certs")).unwrap();
    let files = ["certs/me.pem", "certs/me.key", "certs/pin.pem"];
    for f in files {
        write(
            &dir, f, "one
",
        );
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "certs"]);
    git(&dir, &["config", "http.sslCert", files[0]]);
    git(&dir, &["config", "http.sslKey", files[1]]);
    git(&dir, &["config", "http.pinnedPubkey", files[2]]);
    let saved = scan_at(&dir).unwrap().print();
    for f in files {
        write(
            &dir, f, "two
",
        );
        assert!(!saved.covers(&scan_at(&dir).unwrap().print()), "{f} changed");
        write(
            &dir, f, "one
",
        );
        assert!(saved.covers(&scan_at(&dir).unwrap().print()), "{f} put back");
    }
    // Hashes name no file.
    git(&dir, &["config", "http.pinnedPubkey", "sha256//AAAA"]);
    let s = scan_at(&dir).unwrap();
    assert!(!s.follows.iter().any(|f| f.path == files[2]), "{:?}", s.follows);
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

#[test]
fn a_path_read_back_from_its_text_has_the_same_bytes() {
    for text in ["tools/x.sh", "caf\u{e9}/x.sh", "odd\u{FFFD}name/\u{FFFD}FF.sh", ""] {
        assert_eq!(path_of_text(&text_of(text.as_bytes())), Some(std::path::PathBuf::from(text)), "{text:?}");
    }
    // A mark not written by `text_of` names nothing.
    assert_eq!(path_of_text("x\u{FFFD}"), None);
    assert_eq!(path_of_text("x\u{FFFD}G1"), None);
}

#[cfg(unix)]
#[test]
fn a_path_that_isn_t_utf8_is_read_by_its_bytes() {
    use std::os::unix::ffi::OsStrExt;
    for bytes in [&b"tools/\xff.sh"[..], b"\xfe\xff", b"a\xef\xbf\xbd\xc3/x"] {
        let path = path_of_text(&text_of(bytes)).expect("a path holds any bytes here");
        assert_eq!(path.as_os_str().as_bytes(), bytes);
    }
}

#[cfg(not(unix))]
#[test]
fn a_path_that_isn_t_utf8_names_nothing_here() {
    assert_eq!(path_of_text(&text_of(b"tools/\xff.sh")), None);
}

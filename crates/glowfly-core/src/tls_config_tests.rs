use super::*;
use crate::testutil::{empty_global_config, git, scratch_repo};

fn file(scope: Option<Scope>, path: &str) -> Origin {
    Origin::File { scope, path: path.to_owned() }
}

fn listed(scope: Option<Scope>, origin: Origin, key: &str, value: Option<&str>) -> Listed {
    Listed { scope, origin, key: key.to_owned(), value: value.map(str::to_owned) }
}

fn setting(value: &str, origin: Origin, url: Option<&str>) -> Option<Setting> {
    Some(Setting { value: value.to_owned(), origin, url: url.map(str::to_owned) })
}

#[test]
fn parse_listing_reads_scopes_origins_and_bare_keys() {
    let out = "system\0file:C:/Program Files/Git/etc/gitconfig\0http.sslbackend\nschannel\0local\0file:.git/config\0http.https://h.example/.sslcainfo\ncerts/corp.pem\0command\0command line:\0http.schannelusesslcainfo\0";
    assert_eq!(
        parse_listing(out, true),
        [
            listed(
                Some(Scope::System),
                file(Some(Scope::System), "C:/Program Files/Git/etc/gitconfig"),
                "http.sslbackend",
                Some("schannel")
            ),
            listed(
                Some(Scope::Local),
                file(Some(Scope::Local), ".git/config"),
                "http.https://h.example/.sslcainfo",
                Some("certs/corp.pem")
            ),
            listed(Some(Scope::Command), Origin::CommandLine, "http.schannelusesslcainfo", None),
        ]
    );
    let old = "file:/etc/gitconfig\0http.sslbackend\nopenssl\0command line:\0http.sslcainfo\n/x.pem\0";
    assert_eq!(
        parse_listing(old, false),
        [
            listed(None, file(None, "/etc/gitconfig"), "http.sslbackend", Some("openssl")),
            listed(None, Origin::CommandLine, "http.sslcainfo", Some("/x.pem")),
        ]
    );
    assert!(parse_listing("", true).is_empty());
}

#[test]
fn resolve_takes_the_value_in_effect_and_where_it_is_set() {
    let sys = file(Some(Scope::System), "/etc/gitconfig");
    let global = file(Some(Scope::Global), "/home/ada/.gitconfig");
    let local = file(Some(Scope::Local), ".git/config");
    let all = [
        listed(Some(Scope::System), sys.clone(), "http.sslbackend", Some("openssl")),
        listed(Some(Scope::Global), global.clone(), "http.sslbackend", Some("schannel")),
        listed(Some(Scope::Global), global.clone(), "http.sslcainfo", Some("/g.pem")),
        listed(Some(Scope::Local), local.clone(), "http.https://h.example/.sslcainfo", Some("certs/corp.pem")),
        listed(Some(Scope::Local), local.clone(), "http.schannelusesslcainfo", None),
    ];
    // No URL: the last plain entry wins, URL-specific ones don't apply.
    let plain = resolve(&all, None);
    assert_eq!(plain.backend, setting("schannel", global.clone(), None));
    assert_eq!(plain.ca_info, setting("/g.pem", global.clone(), None));
    assert_eq!(plain.use_ca_info, setting("true", local.clone(), None), "a bare key is true");
    assert_eq!(plain.check_revoke, None);
    // For a URL, git's own matching decides; the URL-specific key's origin and URL are kept.
    let e = |k: &str, v: &str| (k.to_owned(), Some(v.to_owned()));
    let effective = [e("http.sslbackend", "schannel"), e("http.sslcainfo", "certs/corp.pem")];
    let at = resolve(&all, Some(&effective));
    assert_eq!(at.backend, setting("schannel", global, None));
    assert_eq!(at.ca_info, setting("certs/corp.pem", local, Some("https://h.example/")));
    assert_eq!(at.use_ca_info, None, "missing from the effective listing");
}

#[test]
fn ca_info_ignored_truth_table() {
    let s = |v: &str| setting(v, Origin::Other(String::new()), None);
    let base = TlsSettings { backend: s("schannel"), ca_info: s("/c.pem"), ..TlsSettings::default() };
    assert!(base.ca_info_ignored());
    assert!(!TlsSettings { backend: s("Schannel"), ..base.clone() }.ca_info_ignored(), "git compares the name exactly");
    assert!(!TlsSettings { use_ca_info: s("yes"), ..base.clone() }.ca_info_ignored());
    assert!(!TlsSettings { use_ca_info: s("true"), ..base.clone() }.ca_info_ignored(), "a bare key reads as true");
    assert!(TlsSettings { use_ca_info: s("nonsense"), ..base.clone() }.ca_info_ignored());
    let env_only = TlsSettings { ca_info: None, ca_info_env: Some("/e.pem".into()), ..base.clone() };
    assert!(env_only.ca_info_ignored());
    assert!(!TlsSettings { backend: s("openssl"), ..base.clone() }.ca_info_ignored());
    assert!(!TlsSettings { backend: s("openssl"), ca_info_env: Some("/e.pem".into()), ..base }.ca_info_ignored());
}

#[test]
fn best_effort_needs_git_2_43() {
    assert!(best_effort_supported(None));
    assert!(!best_effort_supported(Some((2, 42, 9))));
    assert!(best_effort_supported(Some((2, 43, 0))));
}

#[test]
fn read_reports_values_and_where_they_are_set() {
    let dir = scratch_repo("tls-config-read");
    let env = empty_global_config(&dir);
    let global = std::path::PathBuf::from(&env[0].1);
    std::fs::write(&global, "[http]\n\tsslBackend = schannel\n").unwrap();
    git(&dir, &["config", "http.https://h.example/.sslCAInfo", "certs/corp.pem"]);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let at = read(&dir, Some("https://h.example/r.git"), &env).unwrap();
    let backend = at.backend.clone().expect("the global backend");
    assert_eq!(backend.value, "schannel");
    let Origin::File { scope, path } = &backend.origin else { panic!("{:?}", backend.origin) };
    assert_eq!(*scope, Some(Scope::Global));
    assert!(path.ends_with("gitconfig"), "{path}");
    let ca = at.ca_info.clone().expect("the URL's CA file");
    assert_eq!(ca.value, "certs/corp.pem");
    assert!(matches!(ca.origin, Origin::File { scope: Some(Scope::Local), .. }), "{:?}", ca.origin);
    assert_eq!(ca.url.as_deref(), Some("https://h.example/"));
    assert_eq!(at.use_ca_info, None);
    assert!(at.ca_info_ignored());

    let plain = read(&dir, None, &env).unwrap();
    assert_eq!(plain.ca_info, None, "the URL's key needs the URL");
    assert_eq!(plain.backend.map(|b| b.value).as_deref(), Some("schannel"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&global);
}

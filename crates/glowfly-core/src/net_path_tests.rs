//! Which spellings name a network place on each OS (pure, every row on every runner), the
//! same-root rule, and the link walk on a real file system.

use super::*;
use crate::testutil::net_place;

fn share(server: &str, share: &str) -> NetRoot {
    NetRoot::Share { server: server.to_owned(), share: share.to_owned() }
}

#[test]
fn windows_spellings_that_reach_a_share() {
    for (text, server, name) in [
        (r"\\nas\team\x", "nas", "team"),
        ("//nas/team/x", "nas", "team"),
        (r"\/nas\team", "nas", "team"),
        (r"/\nas/team", "nas", "team"),
        (r"\\?\UNC\nas\team\x", "nas", "team"),
        (r"\\?\unc\NAS\Team", "nas", "team"),
        (r"\\.\UNC\nas\team\x", "nas", "team"),
        (r"\??\UNC\nas\team\x", "nas", "team"),
        (r"\\localhost\c$\x", "localhost", "c$"),
        (r"\\nas", "nas", ""),
    ] {
        assert_eq!(spelled_root_for(text, true), Some(share(server, name)), "{text}");
    }
}

#[test]
fn windows_device_paths_are_never_allowed() {
    for text in [r"\\?\GLOBALROOT\Device\Mup\nas\team", r"\\.\pipe\x", r"\\.\Mup\nas\team", r"\\?\HarddiskVolume3\x", r"\\\x"] {
        assert_eq!(spelled_root_for(text, true), Some(NetRoot::Device), "{text}");
    }
    // Forward slashes don't make a verbatim path: a share on the server `?`.
    assert_eq!(spelled_root_for("//?/C:/x", true), Some(share("?", "c:")));
    assert!(!may_follow_for(Some(&NetRoot::Device), Some(&NetRoot::Device)));
}

#[test]
fn windows_local_spellings() {
    for text in [
        r"C:\x",
        "c:/x",
        r"\\?\C:\x",
        r"\\.\C:\x",
        r"\??\C:\x",
        r"\\?\Volume{0f1e2d3c-0000-0000-0000-000000000000}\x",
        r"a\b",
        r"\rooted",
        "C:rel",
    ] {
        assert_eq!(root_for(text, true, &|_| false), None, "{text}");
    }
}

#[test]
fn wsl_host_names_are_one_root() {
    let wsl = spelled_root_for(r"\\wsl$\Ubuntu\home", true).unwrap();
    assert_eq!(spelled_root_for(r"\\WSL.localhost\ubuntu\x", true).as_ref(), Some(&wsl));
    assert_eq!(spelled_root_for(r"\\?\UNC\wsl.localhost\Ubuntu\x", true).as_ref(), Some(&wsl));
    let debian = spelled_root_for(r"\\wsl$\Debian", true).unwrap();
    assert_ne!(debian, wsl);
    assert!(!may_follow_for(Some(&wsl), None), "from a repository on C: it would start the distro");
    assert!(may_follow_for(Some(&wsl), Some(&wsl)), "inside the same distro");
    assert!(!may_follow_for(Some(&debian), Some(&wsl)));
}

#[test]
fn mapped_drive_letters_count_as_network() {
    let z = |c: char| c == 'Z';
    for text in [r"Z:\x", "z:/x", r"\\?\Z:\x"] {
        assert_eq!(root_for(text, true, &z), Some(NetRoot::MappedDrive('Z')), "{text}");
    }
    assert_eq!(root_for(r"C:\x", true, &z), None);
    assert_eq!(spelled_root_for(r"Z:\x", true), None, "the spelling alone never maps a letter");
    let mapped = NetRoot::MappedDrive('Z');
    assert!(may_follow_for(Some(&mapped), Some(&NetRoot::MappedDrive('Z'))));
    assert!(!may_follow_for(Some(&mapped), Some(&NetRoot::MappedDrive('Y'))));
    assert!(!may_follow_for(Some(&mapped), None));
}

#[test]
fn unix_spellings() {
    let host = Some(NetRoot::Automount("host".to_owned()));
    for text in
        ["/net/host/x", "//net/host/x", "/net/./host", "/Network/Servers/host/x", "/System/Volumes/Data/net/host/x", "/tmp/../net/host"]
    {
        assert_eq!(spelled_root_for(text, false), host, "{text}");
    }
    // macOS's root volume ignores case by default: every spelling reaches the automounter.
    for text in [
        "/NET/host/x",
        "/Net/HOST",
        "/network/servers/host/x",
        "/NETWORK/SERVERS/Host",
        "/system/volumes/data/NET/host",
        "/SYSTEM/Volumes/DATA/Network/Servers/host",
    ] {
        assert_eq!(spelled_root_for(text, false), host, "{text}");
    }
    for text in ["/home/me", "//server/share/x", r"\\server\share\x", "/net", "/netx/y", "net/host", "/Network/Servers"] {
        assert_eq!(spelled_root_for(text, false), None, "{text}");
    }
}

#[test]
fn same_root_rules() {
    let nas = spelled_root_for(r"\\nas\team", true);
    assert!(may_follow_for(None, None));
    assert!(may_follow_for(None, nas.as_ref()), "a local place from a share");
    assert!(!may_follow_for(nas.as_ref(), None), "a share from a local place");
    assert!(may_follow_for(spelled_root_for(r"\\?\UNC\nas\team", true).as_ref(), spelled_root_for(r"\\NAS\Team", true).as_ref()));
    assert!(!may_follow_for(spelled_root_for(r"\\nas\other", true).as_ref(), nas.as_ref()), "another share on the server");
    assert!(!may_follow_for(spelled_root_for(r"\\files\team", true).as_ref(), nas.as_ref()), "another server");
    let host = spelled_root_for("/net/host/a", false);
    assert!(may_follow_for(spelled_root_for("/net/host/b", false).as_ref(), host.as_ref()));
    assert!(!may_follow_for(spelled_root_for("/net/other/b", false).as_ref(), host.as_ref()));
}

#[test]
fn a_name_holding_a_nul_is_refused() {
    let home = crate::paths::temp_dir();
    // The OS is never asked about it, so whatever it leads to can't be checked.
    assert!(!may_follow(&home.join("x\0y"), &home));
    assert!(!walk(&home.join("x\0y"), &home, None));
}

#[test]
fn links_to_a_network_place_are_refused() {
    let home = crate::paths::temp_dir().join(format!("glowfly-test-net-path-links-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let other = home.with_file_name(format!("{}-other", home.file_name().unwrap().to_string_lossy()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(other.join("sub")).unwrap();
    assert!(!may_follow(&net_place().join("x"), &home), "the spelling alone");
    assert!(may_follow(&home.join("missing/deeper/x"), &home), "a part that doesn't exist ends the walk");
    assert!(may_follow(&crate::paths::temp_dir().join("x"), &crate::paths::temp_dir()));
    if local_link(&other, &home.join("local")) {
        assert!(may_follow(&home.join("local/sub/x"), &home), "a link to a local folder");
        assert!(local_link(&home.join("b"), &home.join("a")) && local_link(&home.join("a"), &home.join("b")));
        assert!(!may_follow(&home.join("a/x"), &home), "a loop ends, refused");
    }
    if crate::testutil::link_to(&net_place(), &home.join("link"), true) {
        assert!(!may_follow(&home.join("link/sub"), &home), "a link to a share");
        assert!(!may_follow(&home.join("link"), &home));
        assert!(crate::testutil::link_to(Path::new("link"), &home.join("relative"), true));
        assert!(!may_follow(&home.join("relative/x"), &home), "a relative link to a link to a share");
        if home.join("local").exists() {
            // Win32 drops `local\..` before any I/O; on Unix `..` leaves the folder `local`
            // leads to, where nothing named `link` is.
            assert_eq!(may_follow(&home.join("local/../link/x"), &home), !cfg!(windows));
        }
    }
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&other);
}

/// A folder link `link` to `target`: a symbolic link, or on Windows a junction where symbolic
/// links need rights the account lacks (a junction can't lead to a share).
fn local_link(target: &Path, link: &Path) -> bool {
    crate::testutil::link_to(target, link, true)
        || (cfg!(windows)
            && std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success()))
}

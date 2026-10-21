use super::*;
use crate::forge::mock::{Mock, Reply};
use std::path::PathBuf;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
}

#[test]
fn versions_parse() {
    assert_eq!(v("v1.2.3"), Version { major: 1, minor: 2, patch: 3, pre: vec![] });
    assert_eq!(v("1.2.3"), v("v1.2.3"));
    assert_eq!(v("V0.1.0"), Version { major: 0, minor: 1, patch: 0, pre: vec![] });
    assert_eq!(v("1.2.3-rc.1").pre, vec![PreId::Alpha("rc".into()), PreId::Num(1)]);
    assert_eq!(v("1.2.3+build.5"), v("1.2.3"), "build metadata is ignored");
    for bad in ["", "x", "1.2", "1.2.3.4", "01.2.3", "1.02.3", "1.2.3-", "1.2.3-rc..1", "1.2.3-01", "v", "1.2.x", " 1.2.3", "1.2.3+"] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
    for s in ["1.2.3", "0.3.0-rc.1", "1.0.0-alpha.beta", "10.20.30"] {
        assert_eq!(v(s).to_string(), s);
    }
    assert!(v("0.3.0-rc.1").is_prerelease() && !v("0.3.0").is_prerelease());
}

#[test]
fn versions_follow_semver_precedence() {
    assert!(v("1.10.0") > v("1.9.0"));
    assert!(v("1.0.0") > v("1.0.0-rc.1"));
    assert!(v("1.0.0-rc.2") > v("1.0.0-rc.1"));
    assert!(v("1.0.0-alpha.1") < v("1.0.0-alpha.beta"), "numeric ids sort below alphanumeric ones");
    assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"), "a shorter list sorts first");
    assert!(v("2.0.0") > v("1.99.99"));
    assert_eq!(v("1.0.0").cmp(&v("v1.0.0+x")), std::cmp::Ordering::Equal);
}

#[test]
fn checks_are_due_once_a_day() {
    let now = 1_769_947_200;
    assert!(due(true, None, now), "never checked");
    assert!(!due(true, Some(now - 23 * 3600), now));
    assert!(due(true, Some(now - CHECK_EVERY_SECS), now), "exactly a day");
    assert!(due(true, Some(now + 3600), now), "the clock went back");
    assert!(!due(false, None, now), "turned off");
    assert!(!due(true, None, 0), "no clock yet");
}

const WIN: Platform = Platform { os: TargetOs::Windows, arch: Arch::X86_64 };
const MAC_ARM: Platform = Platform { os: TargetOs::Mac, arch: Arch::Aarch64 };
const MAC_X86: Platform = Platform { os: TargetOs::Mac, arch: Arch::X86_64 };
const LINUX: Platform = Platform { os: TargetOs::Linux, arch: Arch::X86_64 };

fn asset(name: &str) -> Asset {
    Asset { name: name.to_owned(), size: 10, url: format!("https://github.com/bg9m9r/trellis/releases/download/v0.3.0/{name}") }
}

fn release(tag: &str, names: &[&str]) -> Release {
    Release {
        tag: tag.to_owned(),
        name: format!("Glowfly {tag}"),
        html_url: format!("https://github.com/bg9m9r/trellis/releases/tag/{tag}"),
        body: String::new(),
        published_at: None,
        assets: names.iter().map(|n| asset(n)).collect(),
    }
}

/// Every file the Release workflow publishes for v0.3.0.
fn full_release() -> Release {
    let mut names = vec!["SHA256SUMS".to_owned()];
    for (triple, exts) in [
        ("x86_64-pc-windows-msvc", &["zip", "msi"][..]),
        ("aarch64-apple-darwin", &["tar.gz", "dmg"]),
        ("x86_64-apple-darwin", &["tar.gz", "dmg"]),
        ("x86_64-unknown-linux-gnu", &["tar.gz", "deb"]),
    ] {
        for ext in exts {
            names.push(format!("glowfly-v0.3.0-{triple}.{ext}"));
            names.push(format!("glowfly-v0.3.0-{triple}.{ext}.sha256"));
        }
    }
    release("v0.3.0", &names.iter().map(String::as_str).collect::<Vec<_>>())
}

fn picked(r: &Release, p: Platform, k: InstallKind) -> Option<String> {
    pick(r, p, k).map(|a| a.name)
}

#[test]
fn the_file_for_each_platform_and_install() {
    use InstallKind::{Installer, Portable};
    let r = full_release();
    let name = |s: &str| Some(format!("glowfly-v0.3.0-{s}"));
    assert_eq!(picked(&r, WIN, Installer), name("x86_64-pc-windows-msvc.msi"));
    assert_eq!(picked(&r, WIN, Portable), name("x86_64-pc-windows-msvc.zip"));
    assert_eq!(picked(&r, MAC_ARM, Installer), name("aarch64-apple-darwin.dmg"));
    assert_eq!(picked(&r, MAC_X86, Installer), name("x86_64-apple-darwin.dmg"));
    assert_eq!(picked(&r, MAC_ARM, Portable), name("aarch64-apple-darwin.tar.gz"));
    assert_eq!(picked(&r, LINUX, Installer), name("x86_64-unknown-linux-gnu.deb"));
    assert_eq!(picked(&r, LINUX, Portable), name("x86_64-unknown-linux-gnu.tar.gz"));
    // No build for these.
    assert_eq!(picked(&r, Platform { os: TargetOs::Linux, arch: Arch::Aarch64 }, Installer), None);
    assert_eq!(picked(&r, Platform { os: TargetOs::Windows, arch: Arch::Aarch64 }, Portable), None);
}

#[test]
fn picking_falls_back_and_ignores_other_files() {
    use InstallKind::{Installer, Portable};
    let zip_only = release("v0.3.0", &["glowfly-v0.3.0-x86_64-pc-windows-msvc.zip", "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi.sha256"]);
    assert_eq!(picked(&zip_only, WIN, Installer).as_deref(), Some("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip"));
    let deb_only = release("v0.3.0", &["glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb"]);
    assert_eq!(picked(&deb_only, LINUX, Portable).as_deref(), Some("glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb"));
    // Checksums, other tags and other triples never count.
    let other = release(
        "v0.3.0",
        &[
            "SHA256SUMS",
            "glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb.sha256",
            "glowfly-v0.2.0-x86_64-unknown-linux-gnu.deb",
            "glowfly-v0.3.0-aarch64-unknown-linux-gnu.deb",
            "glowfly-v0.3.0-x86_64-unknown-linux-gnu.rpm",
        ],
    );
    assert_eq!(picked(&other, LINUX, Installer), None);
    assert_eq!(picked(&other, LINUX, Portable), None);
}

#[test]
fn files_named_for_trellis_are_picked_too() {
    use InstallKind::Installer;
    let old = release("v0.3.0", &["trellis-v0.3.0-x86_64-pc-windows-msvc.msi", "trellis-v0.3.0-x86_64-pc-windows-msvc.zip"]);
    assert_eq!(picked(&old, WIN, Installer).as_deref(), Some("trellis-v0.3.0-x86_64-pc-windows-msvc.msi"));
    // The new name wins when a release has both.
    let both = release("v0.3.0", &["trellis-v0.3.0-x86_64-pc-windows-msvc.msi", "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"]);
    assert_eq!(picked(&both, WIN, Installer).as_deref(), Some("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"));
}

#[test]
fn how_a_copy_was_installed() {
    use InstallKind::{Installer, Portable};
    let win = [
        (r"C:\Users\ada\AppData\Local\Programs\Glowfly\glowfly.exe", Installer),
        (r"c:\users\ada\appdata\local\PROGRAMS\glowfly\GLOWFLY.EXE", Installer),
        ("C:/Users/ada/AppData/Local/Programs/Glowfly/glowfly.exe", Installer),
        (r"D:\tools\glowfly\glowfly.exe", Portable),
        (r"C:\Programs\Other\glowfly.exe", Portable),
        ("glowfly.exe", Portable),
    ];
    for (exe, want) in win {
        assert_eq!(install_kind_for(exe, TargetOs::Windows), want, "{exe}");
    }
    let mac = [
        ("/Applications/Glowfly.app/Contents/MacOS/glowfly", Installer),
        ("/Users/ada/Applications/Glowfly.app/Contents/MacOS/glowfly", Installer),
        ("/Users/ada/bin/glowfly", Portable),
        ("/Users/ada/.app/Contents/MacOS/glowfly", Portable),
    ];
    for (exe, want) in mac {
        assert_eq!(install_kind_for(exe, TargetOs::Mac), want, "{exe}");
    }
    assert_eq!(install_kind_for("/usr/bin/glowfly", TargetOs::Linux), Installer);
    assert_eq!(install_kind_for("/home/u/.local/bin/glowfly", TargetOs::Linux), Portable);
    assert_eq!(install_kind_for("/usr/local/bin/glowfly", TargetOs::Linux), Portable);
    // Each OS's rule only applies on that OS.
    assert_eq!(install_kind_for("/usr/bin/glowfly", TargetOs::Mac), Portable);
}

#[test]
fn offers_only_newer_releases() {
    let r = full_release();
    let o = offer(&r, &v("0.2.0"), Some(WIN), InstallKind::Installer).expect("newer");
    assert_eq!(o.version, v("0.3.0"));
    assert_eq!(o.asset.as_ref().map(|a| a.name.as_str()), Some("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"));
    assert!(matches!(&o.sums, SumsSource::Combined(a) if a.name == "SHA256SUMS"));
    assert_eq!(offer(&r, &v("0.3.0"), Some(WIN), InstallKind::Installer), None, "the same version");
    assert_eq!(offer(&r, &v("0.4.0"), Some(WIN), InstallKind::Installer), None, "an older one");
    assert_eq!(offer(&release("nightly", &[]), &v("0.2.0"), Some(WIN), InstallKind::Installer), None, "a tag that isn't a version");
    assert!(offer(&r, &v("0.3.0-rc.1"), Some(WIN), InstallKind::Installer).is_some(), "a pre-release is offered its release");
    let none = offer(&r, &v("0.2.0"), None, InstallKind::Portable).expect("still offered, to open the release page");
    assert_eq!((none.asset, none.sums), (None, SumsSource::None));
}

#[test]
fn where_checksums_come_from() {
    let r = full_release();
    let a = asset("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi");
    assert!(matches!(sums_source(&r, &a), SumsSource::Combined(s) if s.name == "SHA256SUMS"));
    let single = release("v0.3.0", &["glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi.sha256"]);
    assert!(matches!(sums_source(&single, &a), SumsSource::Single(s) if s.name == "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi.sha256"));
    assert_eq!(sums_source(&release("v0.3.0", &["glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"]), &a), SumsSource::None);
}

const HASH_A: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

#[test]
fn checksum_files_are_read() {
    let upper = HASH_A.to_uppercase();
    let text = format!(
        "# comment\n\n{HASH_A}  a.msi\r\n{upper} *b.zip\n{}  short\n{HASH_A}x.msi\n{HASH_A}  my file.tar.gz\n{HASH_A}  \n",
        &HASH_A[..63]
    );
    let entries = parse_sums(&text);
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["a.msi", "b.zip", "my file.tar.gz"]);
    assert_eq!(entries[0].1, sha256(b"test"));
    assert_eq!(entries[1].1, entries[0].1, "upper-case hex");
    assert_eq!(expected_sum(&text, "b.zip"), Some(sha256(b"test")));
    assert_eq!(expected_sum(&text, "short"), None);
    assert_eq!(expected_sum(&text, "missing"), None);
    let two = format!("{}  a.msi\n{HASH_A}  a.msi\n", "0".repeat(64));
    assert_eq!(expected_sum(&two, "a.msi"), Some([0; 32]), "the first entry");
}

#[test]
fn release_json_is_parsed_and_cleaned() {
    let json = br###"{
        "tag_name": "v0.3.0", "name": "", "html_url": "https://github.com/bg9m9r/trellis/releases/tag/v0.3.0",
        "body": "## Notes", "published_at": "2026-10-20T10:00:00Z", "draft": false,
        "assets": [
            {"name": "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", "size": 1234, "browser_download_url": "https://github.com/d/a.msi"},
            {"name": "plain.zip", "size": 1, "browser_download_url": "http://github.com/d/plain.zip"},
            {"name": "../x", "size": 1, "browser_download_url": "https://github.com/d/x"},
            {"name": ".hidden", "size": 1, "browser_download_url": "https://github.com/d/h"},
            {"name": "no-link.zip", "size": 1},
            {"name": "SHA256SUMS", "size": 99, "browser_download_url": "https://github.com/d/SHA256SUMS"}
        ]}"###;
    let r = parse_release(json).unwrap();
    assert_eq!((r.tag.as_str(), r.name.as_str(), r.body.as_str()), ("v0.3.0", "v0.3.0", "## Notes"));
    assert_eq!(r.html_url, "https://github.com/bg9m9r/trellis/releases/tag/v0.3.0");
    assert_eq!(r.published_at.as_deref(), Some("2026-10-20T10:00:00Z"));
    let names: Vec<&str> = r.assets.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", "SHA256SUMS"]);
    assert_eq!(r.assets[0].size, 1234);
    let r = parse_release(br#"{"tag_name": "v1.0.0", "html_url": "javascript:alert(1)"}"#).unwrap();
    assert_eq!((r.html_url.as_str(), r.assets.len()), ("", 0));
    assert!(parse_release(b"[]").is_err());
    for bad in ["", ".", "..", "a/b", r"a\b", "C:x", "a b", &"x".repeat(129)] {
        assert!(!asset_name_ok(bad), "{bad:?}");
    }
    assert!(asset_name_ok("glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz") && asset_name_ok(&"x".repeat(128)));
}

#[test]
fn free_names_count_up_before_the_extension() {
    let taken = |list: &'static [&'static str]| move |n: &str| list.contains(&n);
    assert_eq!(free_name("n.msi", taken(&[])), "n.msi");
    assert_eq!(free_name("n.msi", taken(&["n.msi"])), "n (1).msi");
    assert_eq!(free_name("n.msi", taken(&["n.msi", "n (1).msi"])), "n (2).msi");
    assert_eq!(free_name("x.tar.gz", taken(&["x.tar.gz"])), "x (1).tar.gz");
    assert_eq!(free_name("SHA256SUMS", taken(&["SHA256SUMS"])), "SHA256SUMS (1)");
    assert_eq!(split_extension("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi"), ("glowfly-v0.3.0-x86_64-pc-windows-msvc", ".msi"));
    assert_eq!(split_extension("a.b.tar.gz"), ("a.b", ".tar.gz"));
    assert_eq!(split_extension("SHA256SUMS"), ("SHA256SUMS", ""));
}

/// A fresh, empty folder under the temp folder.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-update-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn hosted() -> Feed {
    Feed::Hosted(REPOSITORY.to_owned())
}

const NAME: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi";
const BODY: &str = "pretend installer bytes";

/// An offer for `NAME` (of `size` bytes) with the checksum published as `sums`.
fn download_offer(size: usize, sums: &str) -> Offer {
    let host = "https://github.com/bg9m9r/trellis/releases/download/v0.3.0";
    let asset = Asset { name: NAME.into(), size: size as u64, url: format!("{host}/{NAME}") };
    let sums = match sums {
        "combined" => SumsSource::Combined(Asset { name: "SHA256SUMS".into(), size: 0, url: format!("{host}/SHA256SUMS") }),
        "single" => SumsSource::Single(Asset { name: format!("{NAME}.sha256"), size: 0, url: format!("{host}/{NAME}.sha256") }),
        _ => SumsSource::None,
    };
    Offer { release: release("v0.3.0", &[]), version: v("0.3.0"), asset: Some(asset), sums }
}

fn line_for(body: &str) -> String {
    let d = sha256(body.as_bytes());
    format!("{}  {NAME}\n", d.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// Serves `NAME` as `served`, with SHA256SUMS and the single .sha256 as given (None is a 404).
fn serve(served: &'static str, combined: Option<String>, single: Option<String>) -> Mock {
    Mock::start(move |req| {
        let file = req.path.rsplit('/').next().unwrap_or("");
        let answer = |t: &Option<String>| t.clone().map_or_else(Reply::not_found, Reply::text);
        match file {
            "SHA256SUMS" => answer(&combined),
            f if f == format!("{NAME}.sha256") => answer(&single),
            f if f == NAME => Reply::text(served),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn a_verified_download_is_saved() {
    let dir = scratch("ok");
    let mock = serve(BODY, Some(format!("{}  other.zip\n{}", HASH_A, line_for(BODY))), None);
    let path = download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap();
    assert_eq!(path, dir.join(NAME));
    assert_eq!(std::fs::read(&path).unwrap(), BODY.as_bytes());
    let hosts: Vec<String> = mock.requests().iter().map(|r| r.host.clone()).collect();
    assert!(hosts.iter().all(|h| h == "github.com"), "{hosts:?}");
    assert!(mock.requests().iter().all(|r| r.header("Authorization").is_none()));
    // Again: the same file is reused; a different one gets a free name.
    assert_eq!(download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap(), dir.join(NAME));
    std::fs::write(dir.join(NAME), "something else").unwrap();
    let again = download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap();
    assert_eq!(again, dir.join("glowfly-v0.3.0-x86_64-pc-windows-msvc (1).msi"));
    assert_eq!(std::fs::read_to_string(dir.join(NAME)).unwrap(), "something else", "never overwritten");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_mismatched_download_leaves_nothing() {
    let dir = scratch("bad");
    let _mock = serve(BODY, Some(line_for("the real installer bytes")), None);
    let err = download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap_err();
    assert!(err.contains("didn't match its published checksum"), "{err}");
    let left: Vec<_> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    assert!(left.is_empty(), "no file and no temp file: {left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_single_checksum_file_is_the_fallback() {
    let dir = scratch("single");
    let mock = serve(BODY, None, Some(line_for(BODY)));
    assert_eq!(download(&hosted(), &download_offer(BODY.len(), "single"), &dir).unwrap(), dir.join(NAME));
    assert_eq!(mock.requests().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_checksum_means_no_download() {
    let dir = scratch("nosum");
    let mock = serve(BODY, None, None);
    let err = download(&hosted(), &download_offer(BODY.len(), "none"), &dir).unwrap_err();
    assert!(err.contains("doesn't publish a checksum") && err.contains(NAME), "{err}");
    assert!(mock.requests().is_empty(), "nothing fetched");
    // Published, but the server has none: the file isn't fetched either.
    let err = download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap_err();
    assert!(err.contains("checksums"), "{err}");
    assert!(mock.requests().iter().all(|r| !r.path.ends_with(".msi")), "the file itself was never asked for");
    // A checksum file that doesn't list the file.
    drop(mock);
    let mock = serve(BODY, Some(format!("{HASH_A}  other.zip\n")), None);
    let err = download(&hosted(), &download_offer(BODY.len(), "combined"), &dir).unwrap_err();
    assert!(err.contains("doesn't publish a checksum"), "{err}");
    assert_eq!(mock.requests().len(), 1);
    assert!(!dir.exists());
}

#[test]
fn a_download_of_the_wrong_size_is_refused() {
    let dir = scratch("size");
    let _mock = serve(BODY, Some(line_for(BODY)), None);
    assert!(download(&hosted(), &download_offer(BODY.len() - 1, "combined"), &dir).is_err(), "longer than announced");
    assert!(download(&hosted(), &download_offer(BODY.len() + 1, "combined"), &dir).unwrap_err().contains("bytes instead of"));
    assert!(!dir.join(NAME).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_latest_release_is_asked_for_without_credentials() {
    let json = r#"{"tag_name": "v0.3.0", "html_url": "https://github.com/bg9m9r/trellis/releases/tag/v0.3.0", "body": "x", "assets": []}"#;
    let mock = Mock::start(move |_| Reply::json(json));
    let r = crate::forge::latest_release("https://github.com/bg9m9r/trellis").unwrap().expect("a release");
    assert_eq!(r.tag, "v0.3.0");
    let req = &mock.requests()[0];
    assert_eq!(req.route(), "GET api.github.com/repos/bg9m9r/trellis/releases/latest");
    assert_eq!(req.header("User-Agent"), Some(concat!("Glowfly/", env!("CARGO_PKG_VERSION"))));
    assert_eq!(req.header("Accept"), Some("application/vnd.github+json"));
    assert_eq!(req.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    assert_eq!(req.header("Authorization"), None);
    drop(mock);

    let _mock = Mock::start(|_| Reply::not_found());
    assert_eq!(crate::forge::latest_release("https://github.com/bg9m9r/trellis").unwrap(), None, "no releases yet");
    drop(_mock);
    let _mock = Mock::start(|_| Reply::status(403, r#"{"message":"API rate limit exceeded for 1.2.3.4."}"#));
    let err = crate::forge::latest_release("https://github.com/bg9m9r/trellis").unwrap_err().to_string();
    assert!(err.contains("rate limit") && err.contains("API rate limit exceeded"), "{err}");
    drop(_mock);
    assert!(crate::forge::latest_release("https://gitlab.com/o/r").is_err());
    assert!(crate::forge::fetch_public("http://github.com/x", 10).is_err(), "https only");
}

#[test]
fn the_releases_api_of_a_repository() {
    let want = Some("https://api.github.com/repos/o/r/releases/latest".to_owned());
    for repo in ["https://github.com/o/r", "https://github.com/o/r.git", "https://github.com/o/r/"] {
        assert_eq!(crate::forge::releases_api(repo), want, "{repo}");
    }
    for repo in ["https://gitlab.com/o/r", "garbage", "", "https://github.com/o", "https://github.com/o/r/x", "https://github.com/o/.."] {
        assert_eq!(crate::forge::releases_api(repo), None, "{repo}");
    }
    assert_eq!(releases_page("https://github.com/o/r.git").as_deref(), Some("https://github.com/o/r/releases"));
    assert!(REPOSITORY.starts_with("https://"));
}

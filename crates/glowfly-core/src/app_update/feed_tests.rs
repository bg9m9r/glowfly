//! A local feed folder (GLOWFLY_UPDATE_FEED) and the headless check. No mock server runs in the
//! folder tests, so any network call would fail them.

use super::*;
use crate::forge::mock::{Mock, Reply};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const WIN: Platform = Platform { os: TargetOs::Windows, arch: Arch::X86_64 };
const MSI: &str = "glowfly-v0.1.1-x86_64-pc-windows-msvc.msi";
const ZIP: &str = "glowfly-v0.1.1-x86_64-pc-windows-msvc.zip";
const MSI_BYTES: &[u8] = b"pretend msi bytes";
const ZIP_BYTES: &[u8] = b"pretend zip archive bytes";

fn v(s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
}

/// A fresh, empty folder under the temp folder.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-feed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn sum_line(name: &str, bytes: &[u8]) -> String {
    format!("{}  {name}\n", hex(sha256(bytes)))
}

/// Writes `files` into `dir` with a release.json for `tag` listing them, plus SHA256SUMS
/// (listed too) when `sums` is Some.
fn write_feed(dir: &Path, tag: &str, files: &[(&str, &[u8])], sums: Option<String>) {
    let mut names: Vec<&str> = Vec::new();
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).unwrap();
        names.push(name);
    }
    if let Some(sums) = sums {
        std::fs::write(dir.join("SHA256SUMS"), sums).unwrap();
        names.push("SHA256SUMS");
    }
    let assets: Vec<serde_json::Value> = names.iter().map(|n| serde_json::json!({ "name": n })).collect();
    let json = serde_json::json!({ "tag_name": tag, "name": format!("Glowfly {tag}"), "body": "notes", "assets": assets });
    std::fs::write(dir.join(FEED_RELEASE), serde_json::to_vec(&json).unwrap()).unwrap();
}

/// The usual feed: the msi and zip of v0.1.1 and a SHA256SUMS covering both.
fn standard_feed(dir: &Path) {
    let sums = format!("{}{}", sum_line(MSI, MSI_BYTES), sum_line(ZIP, ZIP_BYTES));
    write_feed(dir, "v0.1.1", &[(MSI, MSI_BYTES), (ZIP, ZIP_BYTES)], Some(sums));
}

fn is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir).map(|mut d| d.next().is_none()).unwrap_or(true)
}

fn offer_for(feed: &Feed, kind: InstallKind) -> Offer {
    let release = feed.latest().unwrap().expect("a release");
    offer(&release, &v("0.1.0"), Some(WIN), kind).expect("newer")
}

#[test]
fn the_old_trellis_variable_is_read_when_the_new_one_is_unset() {
    let env = |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.into());
    assert_eq!(feed::feed_var(env(&[])), None);
    assert_eq!(feed::feed_var(env(&[("TRELLIS_UPDATE_FEED", "/old")])), Some("/old".into()));
    assert_eq!(feed::feed_var(env(&[("GLOWFLY_UPDATE_FEED", "/new"), ("TRELLIS_UPDATE_FEED", "/old")])), Some("/new".into()));
    assert_eq!(feed::feed_var(env(&[("GLOWFLY_UPDATE_FEED", ""), ("TRELLIS_UPDATE_FEED", "/old")])), Some("/old".into()));
    assert_eq!(feed::feed_var(env(&[("TRELLIS_UPDATE_FEED", "")])), None);
}

#[test]
fn the_feed_comes_from_the_variable() {
    let hosted = Ok(Feed::Hosted(REPOSITORY.to_owned()));
    assert_eq!(Feed::from_value(None, REPOSITORY), hosted);
    assert_eq!(Feed::from_value(Some(OsStr::new("")), REPOSITORY), hosted);
    let err = Feed::from_value(Some(OsStr::new("feed")), REPOSITORY).unwrap_err();
    assert!(err.contains("GLOWFLY_UPDATE_FEED") && err.contains("absolute") && err.contains("\"feed\""), "{err}");
    let dir = crate::paths::temp_dir().join("glowfly-feed");
    assert_eq!(Feed::from_value(Some(dir.as_os_str()), REPOSITORY), Ok(Feed::Folder(dir.clone())));
    let rooted = Feed::from_value(Some(OsStr::new("/feed")), REPOSITORY);
    if cfg!(windows) {
        assert!(rooted.is_err(), "no drive: relative on Windows");
    } else {
        assert_eq!(rooted, Ok(Feed::Folder(PathBuf::from("/feed"))));
    }
    assert_eq!(Feed::Folder(dir.clone()).describe(), format!("folder {}", crate::native_path::shown(&dir)));
    assert_eq!(Feed::Hosted("https://github.com/o/r".into()).describe(), "hosted https://github.com/o/r");
}

#[test]
fn a_folder_feed_lists_files_from_disk() {
    let dir = scratch("list");
    std::fs::write(dir.join(MSI), MSI_BYTES).unwrap();
    std::fs::write(dir.join(ZIP), ZIP_BYTES).unwrap();
    std::fs::write(dir.join("SHA256SUMS"), "x").unwrap();
    std::fs::write(dir.join(".hidden"), "x").unwrap();
    std::fs::create_dir_all(dir.join("glowfly-sub")).unwrap();
    let json = serde_json::json!({
        "tag_name": "v0.1.1", "name": "Glowfly v0.1.1", "body": "## Notes", "html_url": "http://example.com/r",
        "assets": [
            { "name": MSI, "size": 1, "browser_download_url": "https://github.com/d/a.msi" },
            { "name": ZIP }, { "name": "SHA256SUMS" }, { "name": "missing.zip" },
            { "name": "../evil.msi" }, { "name": ".hidden" }, { "name": "glowfly-sub" }
        ]
    });
    std::fs::write(dir.join(FEED_RELEASE), serde_json::to_vec(&json).unwrap()).unwrap();
    let r = Feed::Folder(dir.clone()).latest().unwrap().expect("a release");
    assert_eq!((r.tag.as_str(), r.name.as_str(), r.body.as_str()), ("v0.1.1", "Glowfly v0.1.1", "## Notes"));
    assert_eq!(r.html_url, "", "not https");
    let got: Vec<(&str, u64, &str)> = r.assets.iter().map(|a| (a.name.as_str(), a.size, a.url.as_str())).collect();
    assert_eq!(got, [(MSI, MSI_BYTES.len() as u64, ""), (ZIP, ZIP_BYTES.len() as u64, ""), ("SHA256SUMS", 1, "")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_without_release_json_has_no_release() {
    let dir = scratch("empty");
    assert_eq!(Feed::Folder(dir.clone()).latest(), Ok(None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_feed_folder_fails() {
    let parent = scratch("missing");
    let dir = parent.join("nowhere");
    let err = Feed::Folder(dir.clone()).latest().unwrap_err();
    assert!(err.contains(&crate::native_path::shown(&dir)) && err.contains("doesn't exist"), "{err}");
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn bad_release_json_fails() {
    let dir = scratch("badjson");
    for bad in ["{", r#"{"name": "no tag"}"#, "\u{feff}{\"tag_name\": \"v0.1.1\"}"] {
        std::fs::write(dir.join(FEED_RELEASE), bad).unwrap();
        let err = Feed::Folder(dir.clone()).latest().unwrap_err();
        assert!(err.contains(FEED_RELEASE), "{bad}: {err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_oversized_release_json_fails() {
    let dir = scratch("big");
    let mut json = br#"{"tag_name": "v0.1.1"}"#.to_vec();
    json.resize(1024 * 1024 + 1, b' ');
    std::fs::write(dir.join(FEED_RELEASE), json).unwrap();
    let err = Feed::Folder(dir.clone()).latest().unwrap_err();
    assert!(err.contains("larger than expected"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_feed_download_is_verified() {
    let dir = scratch("dl");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    standard_feed(&feed_dir);
    let feed = Feed::Folder(feed_dir);
    for (kind, name, bytes) in [(InstallKind::Installer, MSI, MSI_BYTES), (InstallKind::Portable, ZIP, ZIP_BYTES)] {
        let o = offer_for(&feed, kind);
        assert_eq!(o.asset.as_ref().map(|a| a.name.as_str()), Some(name));
        let path = download(&feed, &o, &dl).unwrap();
        assert_eq!(path, dl.join(name));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_feed_rejects_a_wrong_checksum() {
    let dir = scratch("wrongsum");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    let sums = format!("{}  {MSI}\n{}", "0".repeat(64), sum_line(ZIP, ZIP_BYTES));
    write_feed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES), (ZIP, ZIP_BYTES)], Some(sums));
    let feed = Feed::Folder(feed_dir);
    let err = download(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap_err();
    assert!(err.contains("didn't match its published checksum"), "{err}");
    assert!(is_empty(&dl), "no file and no temp file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_changed_after_listing_is_refused() {
    let dir = scratch("changed");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    standard_feed(&feed_dir);
    let feed = Feed::Folder(feed_dir.clone());
    let o = offer_for(&feed, InstallKind::Installer);
    std::fs::write(feed_dir.join(MSI), [MSI_BYTES, b"!"].concat()).unwrap();
    let err = download(&feed, &o, &dl).unwrap_err();
    assert!(err.contains("larger than expected"), "{err}");
    std::fs::write(feed_dir.join(MSI), &MSI_BYTES[1..]).unwrap();
    let err = download(&feed, &o, &dl).unwrap_err();
    assert!(err.contains("bytes instead of"), "{err}");
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_feed_without_the_files_sum_downloads_nothing() {
    let dir = scratch("nosum");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    write_feed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES), (ZIP, ZIP_BYTES)], Some(sum_line(ZIP, ZIP_BYTES)));
    let feed = Feed::Folder(feed_dir.clone());
    let err = download(&feed, &offer_for(&feed, InstallKind::Installer), &dl).unwrap_err();
    assert!(err.contains("doesn't publish a checksum") && err.contains(MSI), "{err}");
    assert!(is_empty(&dl));
    // Without SHA256SUMS, the file's own .sha256 is used.
    let single = format!("{MSI}.sha256");
    let line = sum_line(MSI, MSI_BYTES);
    std::fs::remove_file(feed_dir.join("SHA256SUMS")).unwrap();
    write_feed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES), (single.as_str(), line.as_bytes())], None);
    let o = offer_for(&feed, InstallKind::Installer);
    assert!(matches!(&o.sums, SumsSource::Single(a) if a.name == single));
    assert_eq!(std::fs::read(download(&feed, &o, &dl).unwrap()).unwrap(), MSI_BYTES);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_and_download_outcomes() {
    let dir = scratch("check");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    let feed = Feed::Folder(feed_dir.clone());
    let check = |current: &str, platform| check_and_download(&feed, &v(current), platform, InstallKind::Installer, &dl);
    assert_eq!(check("0.1.0", Some(WIN)), Ok(Checked::UpToDate { latest: None }), "no release.json");
    standard_feed(&feed_dir);
    assert_eq!(check("0.1.1", Some(WIN)), Ok(Checked::UpToDate { latest: Some("v0.1.1".into()) }), "the same tag");
    assert_eq!(check("0.2.0", Some(WIN)), Ok(Checked::UpToDate { latest: Some("v0.1.1".into()) }), "an older tag");
    let err = check("0.1.0", None).unwrap_err();
    assert!(err.contains("no download for this system"), "{err}");
    assert!(is_empty(&dl));
    match check("0.1.0", Some(WIN)) {
        Ok(Checked::Downloaded { offer, path }) => {
            assert_eq!((offer.version, path), (v("0.1.1"), dl.join(MSI)));
        }
        other => panic!("{other:?}"),
    }
    std::fs::remove_dir_all(&dl).unwrap();
    write_feed(&feed_dir, "nightly", &[(MSI, MSI_BYTES)], Some(sum_line(MSI, MSI_BYTES)));
    assert_eq!(check("0.1.0", Some(WIN)), Ok(Checked::UpToDate { latest: Some("nightly".into()) }), "not a version");
    write_feed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES)], Some(format!("{}  {MSI}\n", "0".repeat(64))));
    let err = check("0.1.0", Some(WIN)).unwrap_err();
    assert!(err.contains("didn't match its published checksum"), "{err}");
    assert!(is_empty(&dl));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hosted_feed_asks_the_hosting_service() {
    let json = r#"{"tag_name": "v0.1.1", "assets": [{"name": "SHA256SUMS", "size": 5, "browser_download_url": "https://github.com/d/SHA256SUMS"}]}"#;
    let mock = Mock::start(move |_| Reply::json(json));
    let r = Feed::Hosted("https://github.com/bg9m9r/glowfly".into()).latest().unwrap().expect("a release");
    assert_eq!(mock.requests()[0].route(), "GET api.github.com/repos/bg9m9r/glowfly/releases/latest");
    let got: Vec<(&str, u64, &str)> = r.assets.iter().map(|a| (a.name.as_str(), a.size, a.url.as_str())).collect();
    assert_eq!(got, [("SHA256SUMS", 5, "https://github.com/d/SHA256SUMS")], "the API's size and https link");
}

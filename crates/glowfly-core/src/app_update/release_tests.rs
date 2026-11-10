//! Drafts and prereleases: a release its JSON marks as one reads as no release, from the hosting
//! service and from a feed folder, so nothing is offered or downloaded.

use super::feed_tests::{MSI, is_empty, scratch, sum_line, write_feed};
use super::release::parse_feed_release;
use super::*;
use crate::forge::mock::{Mock, Reply};

const WIN: Platform = Platform { os: TargetOs::Windows, arch: Arch::X86_64 };
const MSI_BYTES: &[u8] = b"pretend msi bytes";

fn json(draft: &str, prerelease: &str) -> String {
    let flags = [("draft", draft), ("prerelease", prerelease)]
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!(r#", "{key}": {value}"#))
        .collect::<String>();
    format!(r#"{{"tag_name": "v0.3.0", "html_url": "https://github.com/bg9m9r/glowfly/releases/tag/v0.3.0"{flags}}}"#)
}

#[test]
fn drafts_and_prereleases_are_not_releases() {
    for (draft, prerelease) in [("true", "false"), ("false", "true"), ("true", "true"), ("true", ""), ("", "true")] {
        assert_eq!(parse_release(json(draft, prerelease).as_bytes()).unwrap(), None, "draft {draft:?}, prerelease {prerelease:?}");
    }
    for (draft, prerelease) in [("false", "false"), ("null", "null"), ("", "")] {
        let r = parse_release(json(draft, prerelease).as_bytes()).unwrap().expect("published");
        assert_eq!(r.tag, "v0.3.0", "draft {draft:?}, prerelease {prerelease:?}");
    }
    let draft = br#"{"tag_name": "v0.3.0", "draft": true, "assets": [{"name": "a.msi"}]}"#;
    assert_eq!(parse_feed_release(draft, |_| panic!("a draft's files aren't looked at")).unwrap(), None);
}

/// Rewrites the feed folder's release.json with `key` set to true.
fn mark(feed_dir: &std::path::Path, key: &str, on: bool) {
    let path = feed_dir.join(FEED_RELEASE);
    let mut release: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    release[key] = serde_json::Value::Bool(on);
    std::fs::write(&path, serde_json::to_vec(&release).unwrap()).unwrap();
}

#[test]
fn a_feed_folder_skips_a_draft_or_a_prerelease() {
    let dir = scratch("drafts");
    let (feed_dir, dl) = (dir.join("feed"), dir.join("dl"));
    std::fs::create_dir_all(&feed_dir).unwrap();
    let feed = Feed::TestKeyFolder(feed_dir.clone());
    let current = Version::parse("0.1.0").unwrap();
    let check = || check_and_download(&feed, &current, Some(WIN), InstallKind::Installer, &dl);
    for key in ["draft", "prerelease"] {
        write_feed(&feed_dir, "v0.1.1", &[(MSI, MSI_BYTES)], Some(sum_line(MSI, MSI_BYTES)));
        mark(&feed_dir, key, true);
        assert_eq!(feed.latest(), Ok(None), "{key}");
        assert_eq!(check(), Ok(Checked::UpToDate { latest: None }), "{key}");
        assert!(is_empty(&dl), "nothing downloaded for a {key}");
    }
    mark(&feed_dir, "draft", false);
    mark(&feed_dir, "prerelease", false);
    match check() {
        Ok(Checked::Downloaded { path, .. }) => assert_eq!(path, dl.join(MSI)),
        other => panic!("a published release is offered: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_hosted_feed_skips_a_draft_or_a_prerelease() {
    for (draft, prerelease) in [("false", "true"), ("true", "false")] {
        let answer = json(draft, prerelease);
        let mock = Mock::start(move |_| Reply::json(&answer));
        assert_eq!(
            crate::forge::latest_release("https://github.com/bg9m9r/glowfly").unwrap(),
            None,
            "draft {draft}, prerelease {prerelease}"
        );
        drop(mock);
    }
}

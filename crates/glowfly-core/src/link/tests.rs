use super::*;
use std::ffi::OsStr;

const ROOT: &str = "0123456789abcdef0123456789abcdef01234567";

fn link(url: Option<&str>, root: Option<&str>, target: Target) -> Link {
    Link { url: url.map(str::to_owned), root: root.map(str::to_owned), target }
}

fn round_trip(l: &Link) -> String {
    let uri = l.to_uri();
    let parsed = parse(&uri).unwrap_or_else(|e| panic!("{uri}: {e}"));
    assert_eq!(&parsed, l, "{uri}");
    assert_eq!(parsed.to_uri(), uri);
    uri
}

#[test]
fn every_kind_survives_a_round_trip() {
    let url = Some("https://github.com/o/r.git");
    assert_eq!(
        round_trip(&link(url, Some(ROOT), Target::Repository)),
        format!("trellis://repo?url=https%3A%2F%2Fgithub.com%2Fo%2Fr.git&root={ROOT}")
    );
    let uri = round_trip(&link(url, Some(ROOT), Target::Branch("feature/x".into())));
    assert!(uri.starts_with("trellis://branch/feature%2Fx?url="), "{uri}");
    let uri = round_trip(&link(url, None, Target::Branch("café".into())));
    assert!(uri.starts_with("trellis://branch/caf%C3%A9?"), "{uri}");
    round_trip(&link(url, Some(ROOT), Target::Commit(ROOT.into())));
    round_trip(&link(None, Some(ROOT), Target::Commit("ab".repeat(32))));
    round_trip(&link(url, Some(ROOT), Target::Tag("v1.0".into())));
    let uri = round_trip(&link(Some("/srv/my repos/r#1"), Some(ROOT), Target::Repository));
    assert!(uri.contains("url=%2Fsrv%2Fmy%20repos%2Fr%231&"), "{uri}");
    assert_eq!(round_trip(&link(url, None, Target::Repository)), "trellis://repo?url=https%3A%2F%2Fgithub.com%2Fo%2Fr.git");
    assert_eq!(round_trip(&link(None, Some(ROOT), Target::Repository)), format!("trellis://repo?root={ROOT}"));
}

#[test]
fn lenient_forms_are_accepted() {
    let want = link(None, Some(ROOT), Target::Repository);
    assert_eq!(parse(&format!("TRELLIS://repo?root={ROOT}")), Ok(want.clone()));
    assert_eq!(parse(&format!("trellis://repo/?root={ROOT}")), Ok(want.clone()));
    assert_eq!(parse(&format!("trellis://repo?future=1&root={ROOT}&x")), Ok(want.clone()));
    assert_eq!(parse(&format!("trellis://repo?root={ROOT}#line-3")), Ok(want.clone()));
    assert_eq!(parse(&format!("trellis://repo?root={}", ROOT.to_uppercase())), Ok(want));
    let c = parse(&format!("trellis://commit/ABCDEF1?root={ROOT}")).unwrap();
    assert_eq!(c.target, Target::Commit("abcdef1".into()));
    assert_eq!(c.describe(), "commit abcdef1");
}

#[test]
fn malformed_links_are_refused_with_their_reason() {
    let r = format!("root={ROOT}");
    assert_eq!(parse(&format!("https://repo?{r}")), Err(LinkError::NotALink));
    assert_eq!(parse("not a link"), Err(LinkError::NotALink));
    assert_eq!(parse(&format!("trellis://draft/1?{r}")), Err(LinkError::UnknownKind("draft".into())));
    assert_eq!(parse(&format!("trellis://branch?{r}")), Err(LinkError::MissingName));
    assert_eq!(parse(&format!("trellis://branch/?{r}")), Err(LinkError::MissingName));
    assert_eq!(parse(&format!("trellis://commit/xyz?{r}")), Err(LinkError::BadSha("xyz".into())));
    assert_eq!(parse(&format!("trellis://commit/abc12?{r}")), Err(LinkError::BadSha("abc12".into())));
    let long = "a".repeat(65);
    assert_eq!(parse(&format!("trellis://commit/{long}?{r}")), Err(LinkError::BadSha(long)));
    let root39 = &ROOT[..39];
    assert_eq!(parse(&format!("trellis://repo?root={root39}")), Err(LinkError::BadRoot));
    assert_eq!(parse("trellis://repo"), Err(LinkError::NoRepository));
    assert_eq!(parse("trellis://repo?url=a&url=b"), Err(LinkError::Duplicate("url")));
    assert_eq!(parse(&format!("trellis://repo?{r}&{r}")), Err(LinkError::Duplicate("root")));
    assert_eq!(parse(&format!("trellis://branch/%FF?{r}")), Err(LinkError::BadEncoding));
    assert_eq!(parse(&format!("trellis://branch/a%2?{r}")), Err(LinkError::BadEncoding));
    assert_eq!(parse(&format!("trellis://branch/a%0Ab?{r}")), Err(LinkError::BadEncoding));
    assert_eq!(parse(&format!("trellis://repo?{r}\u{7}")), Err(LinkError::BadEncoding));
    assert_eq!(parse(&format!("trellis://branch/a%E2%80%AEb?{r}")), Err(LinkError::BadEncoding));
    for bad in ["a..b", "x.lock", "feature%2F"] {
        let text = format!("trellis://branch/{bad}?{r}");
        assert!(matches!(parse(&text), Err(LinkError::BadRefName(_))), "{text}");
    }
    assert!(matches!(parse(&format!("trellis://tag/x.lock?{r}")), Err(LinkError::BadRefName(_))));
    for bad in ["-oProxy", "ext%3A%3Ash%20-c%20x", "FD%3A%3A3", ""] {
        let text = format!("trellis://repo?url={bad}&{r}");
        assert_eq!(parse(&text), Err(LinkError::BadUrl), "{text}");
    }
    let huge = format!("trellis://repo?{r}&pad={}", "x".repeat(MAX_LINK));
    assert_eq!(parse(&huge), Err(LinkError::TooLong));
    // Every reason reads as a sentence fragment of Trellis's own.
    assert!(LinkError::UnknownKind("draft".into()).to_string().contains("newer Trellis"));
}

#[test]
fn urls_of_one_repository_match() {
    let same = ["https://github.com/O/R.git", "git@github.com:o/r", "ssh://git@ssh.github.com:443/o/r", "https://user@GitHub.com/o/r/"];
    for a in same {
        for b in same {
            assert!(same_url(a, b), "{a} vs {b}");
        }
    }
    assert_eq!(url_key("git@github.com:o/r.git"), "github.com/o/r");
    assert!(!same_url("https://github.com/o/r", "https://github.com/o/s"));
    assert!(!same_url("https://github.com/o/r", "https://gitlab.com/o/r"));
    assert!(!same_url("", ""));
    // Local paths and file URLs: separators, a trailing slash and .git don't matter.
    assert_eq!(url_key("/srv/git/r.git/"), "/srv/git/r");
    assert_eq!(url_key("file:///srv/git/r/"), "/srv/git/r");
    assert!(same_url("file:///srv/git/r/", "/srv/git/r.git"));
    assert!(same_url(r"C:\git\r", "file:///C:/git/r/"));
}

#[test]
fn links_are_told_from_paths() {
    assert!(looks_like_link(OsStr::new(&format!("trellis://repo?root={ROOT}"))));
    assert!(looks_like_link(OsStr::new("TRELLIS://commit/abc")));
    assert!(!looks_like_link(OsStr::new("trellis")));
    assert!(!looks_like_link(OsStr::new("trellis-repo")));
    assert!(!looks_like_link(OsStr::new("/home/me/trellis")));
}

#[test]
fn labels_for_messages() {
    let l = link(None, Some(ROOT), Target::Branch("feature/x".into()));
    assert_eq!(l.describe(), "branch feature/x");
    assert_eq!(l.repo_label(), "the repository with root commit 0123456");
    let l = link(Some("https://h/o/r"), Some(ROOT), Target::Tag("v1".into()));
    assert_eq!((l.describe(), l.repo_label()), ("tag v1".to_owned(), "https://h/o/r".to_owned()));
    assert_eq!(link(None, Some(ROOT), Target::Repository).describe(), "repository");
}

#[test]
fn shared_urls_carry_no_credentials() {
    assert_eq!(shareable_url("https://me:ghp_SECRET@github.com/o/r.git"), "https://github.com/o/r.git");
    assert_eq!(shareable_url("HTTPS://ghp_token@h:8443/o/r?x=@y"), "HTTPS://h:8443/o/r?x=@y");
    assert_eq!(shareable_url("ssh://git:pw@h/o/r"), "ssh://git@h/o/r");
    assert_eq!(shareable_url("ssh://:pw@h/o/r"), "ssh://h/o/r");
    for same in ["ssh://git@h:22/o/r", "git@github.com:o/r.git", "https://h/o@r", "/srv/git/r", "file:///srv/r", "https://h"] {
        assert_eq!(shareable_url(same), same);
    }
    assert!(same_url(&shareable_url("https://u:p@github.com/o/r"), "git@github.com:o/r"));
}

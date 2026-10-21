//! Reading the configured remotes' raw URLs: remotes without branches, several push URLs,
//! insteadOf rewriting and names with '/' or '.'.

use super::*;
use crate::testutil::*;
use std::path::Path;

fn snap(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(10)).unwrap()
}

#[test]
fn remote_info_lists_every_remote() {
    let (work, bare) = with_origin("remotes-every");
    let path = bare.display().to_string();
    git(&work, &["remote", "add", "empty", &path]);
    let s = snap(&work);
    assert!(s.remotes.iter().any(|(r, b)| r == "empty" && b.is_empty()), "{:?}", s.remotes.iter().map(|r| &r.0).collect::<Vec<_>>());
    let names: Vec<&str> = s.remotes.iter().map(|(r, _)| r.as_str()).collect();
    let info_names: Vec<&str> = s.remote_info.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, info_names);
    let empty = s.remote_info.iter().find(|r| r.name == "empty").unwrap();
    assert_eq!(*empty, RemoteInfo { name: "empty".into(), urls: vec![path.clone()], push_urls: vec![], skip_fetch_all: false });
    assert_eq!(empty.fetch_url(), path);
    assert_eq!(empty.push_url(), None);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn remote_info_reads_raw_push_urls() {
    let dir = scratch_repo("remotes-push");
    git(&dir, &["remote", "add", "origin", "https://h/a.git"]);
    git(&dir, &["config", "--add", "remote.origin.pushurl", "https://h/a.git"]);
    git(&dir, &["config", "--add", "remote.origin.pushurl", "https://h/b.git"]);
    let s = snap(&dir);
    let o = &s.remote_info[0];
    assert_eq!(o.push_urls, vec!["https://h/a.git".to_owned(), "https://h/b.git".to_owned()]);
    assert_eq!(o.push_url(), Some("https://h/a.git"));
    assert_eq!(o.separate_push_url(), None);
    let mut other = o.clone();
    other.push_urls = vec!["https://h/b.git".into()];
    assert_eq!(other.separate_push_url(), Some("https://h/b.git"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remote_info_ignores_insteadof() {
    let dir = scratch_repo("remotes-insteadof");
    git(&dir, &["config", "url.https://example.com/.insteadOf", "ex:"]);
    git(&dir, &["remote", "add", "origin", "ex:o/r.git"]);
    let s = snap(&dir);
    assert_eq!(s.remote_info[0].urls, vec!["ex:o/r.git".to_owned()]);
    assert_eq!(s.remote_urls, vec![("origin".to_owned(), "https://example.com/o/r.git".to_owned())]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remote_names_with_slash_and_dot() {
    let dir = scratch_repo("remotes-names");
    git(&dir, &["remote", "add", "team/fork", "https://h/fork.git"]);
    git(&dir, &["remote", "add", "a.b", "https://h/ab.git"]);
    let s = snap(&dir);
    let url = |n: &str| s.remote_info.iter().find(|r| r.name == n).map(|r| r.fetch_url().to_owned());
    assert_eq!(url("team/fork").as_deref(), Some("https://h/fork.git"));
    assert_eq!(url("a.b").as_deref(), Some("https://h/ab.git"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remote_heads_come_from_the_remote_head_ref() {
    let (work, bare) = with_origin("remotes-heads");
    assert!(snap(&work).remote_heads.is_empty(), "a push records no remote HEAD");
    git(&work, &["remote", "set-head", "origin", "main"]);
    assert_eq!(snap(&work).remote_heads, vec![("origin".to_owned(), "main".to_owned())]);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn remote_info_reads_skip_fetch_all() {
    let dir = scratch_repo("remotes-skip-fetch-all");
    git(&dir, &["remote", "add", "origin", "https://h/a.git"]);
    git(&dir, &["remote", "add", "mirror", "https://h/m.git"]);
    set_config(&dir, &[("remote.mirror.skipFetchAll", "true")]);
    let s = snap(&dir);
    let skip = |name: &str| s.remote_info.iter().find(|r| r.name == name).map(|r| r.skip_fetch_all);
    assert_eq!(skip("mirror"), Some(true));
    assert_eq!(skip("origin"), Some(false), "false when unset");
    let _ = std::fs::remove_dir_all(&dir);
}

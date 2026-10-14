//! Loading with hidden and soloed refs: which labels and commits the graph keeps.

use super::*;
use crate::testutil::*;
use std::path::Path;

fn commit(dir: &Path, file: &str, msg: &str) {
    write(dir, file, msg);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

/// main with one commit, and `feature` and `other` each one commit ahead of it; on main.
fn branches(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, "a.txt", "first");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "f.txt", "feature work");
    git(&dir, &["checkout", "-q", "-b", "other", "main"]);
    commit(&dir, "o.txt", "other work");
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn opts(hidden: &[&str], solo: &[&str]) -> LoadOptions {
    LoadOptions {
        max_commits: 100,
        hidden: hidden.iter().map(|s| s.to_string()).collect(),
        solo: solo.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn has(s: &Snapshot, summary: &str) -> bool {
    s.commits.iter().any(|c| c.summary == summary)
}

fn labels(s: &Snapshot) -> Vec<String> {
    let mut v: Vec<String> = s.refs.values().flatten().map(|l| l.name.clone()).collect();
    v.sort();
    v
}

#[test]
fn hidden_branch_drops_label_and_its_own_commits() {
    let dir = branches("filter-hide");
    let s = load(&dir, &opts(&["refs/heads/feature"], &[])).unwrap();
    assert!(!labels(&s).contains(&"feature".to_owned()));
    assert!(!has(&s, "feature work"));
    assert!(has(&s, "other work") && has(&s, "first"));
    assert!(s.locals.iter().any(|b| b.name == "feature"), "the sidebar still lists it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hidden_branch_reachable_from_another_keeps_commits() {
    let dir = branches("filter-merged");
    git(&dir, &["merge", "-q", "--no-ff", "feature", "-m", "merge feature"]);
    let s = load(&dir, &opts(&["refs/heads/feature"], &[])).unwrap();
    assert!(!labels(&s).contains(&"feature".to_owned()));
    assert!(has(&s, "feature work"), "main still reaches it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn solo_shows_soloed_ref_and_head_only() {
    let dir = branches("filter-solo");
    let s = load(&dir, &opts(&[], &["refs/heads/feature"])).unwrap();
    assert!(has(&s, "feature work") && has(&s, "first"));
    assert!(!has(&s, "other work"));
    assert_eq!(labels(&s), ["feature", "main"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn smart_mode_skips_hidden_tips() {
    let (work, bare) = with_origin("filter-smart");
    remote_commit(&bare, "main", "r.txt", "origin only");
    git(&work, &["fetch", "-q", "origin"]);
    let smart = |hidden: &[&str]| LoadOptions { smart: true, ..opts(hidden, &[]) };
    let shown = load(&work, &smart(&[])).unwrap();
    assert!(has(&shown, "origin only"), "the upstream is a smart tip");
    let hidden = load(&work, &smart(&["refs/remotes/origin/main"])).unwrap();
    assert!(!has(&hidden, "origin only"));
    assert!(has(&hidden, "first"));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn checked_out_branch_is_never_hidden() {
    let dir = branches("filter-head");
    git(&dir, &["checkout", "-q", "feature"]);
    for smart in [false, true] {
        let s = load(&dir, &LoadOptions { smart, ..opts(&["refs/heads/feature"], &[]) }).unwrap();
        let head = s.refs.get(&s.head_oid.unwrap()).expect("HEAD row has labels");
        assert!(head.iter().any(|l| l.name == "feature" && l.is_head), "smart={smart}");
        assert!(has(&s, "feature work"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

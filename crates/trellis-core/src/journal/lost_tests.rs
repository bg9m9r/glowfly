//! Finding lost commits with `git fsck`, naming dropped stashes, and the commands that keep them.

use super::lost::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use crate::undo::UndoKind;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

fn repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, &[("gc.auto", "0")]);
    dir
}

fn commit(dir: &Path, text: &str) -> Oid {
    write(dir, "a.txt", text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", text]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn stash_top(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().refname_to_id("refs/stash").unwrap()
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

#[test]
fn parse_reads_dangling_commits_only() {
    let (a, b, c) = (oid(0xaa), oid(0xbb), oid(0xcc));
    let out = format!(
        "dangling blob {c}\ndangling tree {c}\ndangling tag {c}\ndangling commit {a}\nmissing commit {c}\n\
         dangling commit {b}\r\ndangling commit {a}\ndangling commit not-an-id\nchecking objects\n"
    );
    assert_eq!(parse(&out), vec![a, b]);
    assert!(parse("").is_empty());
}

#[test]
fn a_reset_away_commit_and_a_dropped_stash_are_both_found() {
    let dir = repo("journal-lost-both");
    let one = commit(&dir, "one");
    let two = commit(&dir, "two");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    write(&dir, "a.txt", "changed");
    git(&dir, &["stash", "push", "-q", "-m", "keep"]);
    let stash = stash_top(&dir);
    git(&dir, &["stash", "drop", "-q"]);
    let found = find(&dir).unwrap();
    assert!(!found.capped && !found.incomplete, "{found:#?}");
    let ids: Vec<Oid> = found.commits.iter().map(|c| c.oid).collect();
    assert_eq!(ids.len(), 2, "{found:#?}");
    let lost_two = found.commits.iter().find(|c| c.oid == two).expect("two is lost");
    assert_eq!((&lost_two.kind, lost_two.summary.as_str(), lost_two.author.as_str()), (&LostKind::Commit, "two", "Test"));
    let s = found.commits.iter().find(|c| c.oid == stash).expect("the stash is lost");
    assert_eq!(s.kind, LostKind::Stash { base: one, branch: Some("main".into()), untracked: false });
    assert_eq!(s.summary, "On main: keep");
    assert!(found.commits.windows(2).all(|w| w[0].time >= w[1].time), "newest first");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn listed_stashes_are_never_lost() {
    let dir = repo("journal-lost-listed");
    commit(&dir, "one");
    write(&dir, "a.txt", "first");
    git(&dir, &["stash", "push", "-q", "-m", "first"]);
    write(&dir, "a.txt", "second");
    git(&dir, &["stash", "push", "-q", "-m", "second"]);
    let found = find(&dir).unwrap();
    assert!(!found.commits.iter().any(|c| matches!(c.kind, LostKind::Stash { .. })), "{found:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untracked_stash_is_named() {
    let dir = repo("journal-lost-untracked");
    commit(&dir, "one");
    write(&dir, "new.txt", "new");
    git(&dir, &["stash", "push", "-q", "-u"]);
    let stash = stash_top(&dir);
    git(&dir, &["stash", "drop", "-q"]);
    let found = find(&dir).unwrap();
    let s = found.commits.iter().find(|c| c.oid == stash).expect("the stash is lost");
    assert!(matches!(&s.kind, LostKind::Stash { untracked: true, branch: Some(b), .. } if b == "main"), "{s:#?}");
    assert!(s.summary.starts_with("WIP on main: "), "{}", s.summary);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_without_commits_has_none() {
    let dir = repo("journal-lost-empty");
    let found = find(&dir).unwrap();
    assert!(found.commits.is_empty(), "{found:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn store_puts_a_dropped_stash_back() {
    let dir = repo("journal-lost-store");
    commit(&dir, "one");
    write(&dir, "a.txt", "changed");
    git(&dir, &["stash", "push", "-q", "-m", "keep"]);
    let stash = stash_top(&dir);
    git(&dir, &["stash", "drop", "-q"]);
    let s = find(&dir).unwrap().commits.into_iter().find(|c| c.oid == stash).expect("lost");
    let (ok, out) = run(&dir, &store_stash(s.oid, &s.summary));
    assert!(ok, "{out}");
    let (ok, list) = run(&dir, &args(&["stash", "list"]));
    assert!(ok && list.contains("On main: keep"), "{list}");
    assert!(!find(&dir).unwrap().commits.iter().any(|c| c.oid == stash), "kept again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commands() {
    let o = oid(0x1a);
    let id = o.to_string();
    assert_eq!(fsck_args(), args(&["fsck", "--connectivity-only", "--no-reflogs", "--no-progress"]));
    assert!(!fsck_args().iter().any(|a| a == "--lost-found"));
    assert_eq!(apply_stash(o), args(&["stash", "apply", &id]));
    assert_eq!(store_stash(o, "On main: keep"), args(&["stash", "store", "-m", "On main: keep", &id]));
    assert_eq!(UndoKind::classify(&apply_stash(o)), Some(UndoKind::StashApply { pop: false, rev: Some(id) }));
}

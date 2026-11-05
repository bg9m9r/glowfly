//! Tests of what `missing` can still find.

use super::super::super::fixture::{Fixture, commit_files, git, write};
use super::missing;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn repo(name: &str) -> (Fixture, PathBuf) {
    let fx = Fixture::new(&format!("reach-{name}"));
    let dir = fx.work().join("r");
    fx.init(&dir, &[("a.txt", "base\n")]);
    (fx, dir)
}

fn gone(dir: &std::path::Path, marker: &str) -> Vec<String> {
    missing(dir, &set(&[marker]), &BTreeSet::new())
}

#[test]
fn found_in_work_tree_index_commit_reflog_stash() {
    let (_fx, dir) = repo("five");

    write(&dir, "a.txt", "base\nedit-1-1\n");
    assert!(gone(&dir, "edit-1-1").is_empty(), "work tree");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    assert_eq!(gone(&dir, "edit-1-1"), ["edit-1-1"], "not anywhere yet");

    write(&dir, "new.txt", "edit-1-2\n");
    git(&dir, &["add", "new.txt"]);
    std::fs::remove_file(dir.join("new.txt")).unwrap();
    assert!(gone(&dir, "edit-1-2").is_empty(), "index");
    git(&dir, &["rm", "-q", "--cached", "new.txt"]);

    commit_files(&dir, &[("a.txt", "base\nedit-1-3\n")], "keep");
    assert!(gone(&dir, "edit-1-3").is_empty(), "commit");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    assert!(gone(&dir, "edit-1-3").is_empty(), "reflog");

    write(&dir, "a.txt", "base\nedit-1-4\n");
    git(&dir, &["stash"]);
    assert!(gone(&dir, "edit-1-4").is_empty(), "stash");
    assert_eq!(gone(&dir, "edit-1-5"), ["edit-1-5"]);
}

#[test]
fn found_in_glowfly_refs() {
    let (_fx, dir) = repo("refs");
    let repository = git2::Repository::open(&dir).unwrap();
    let blob = repository.blob(b"edit-2-1\n").unwrap();
    repository.reference("refs/glowfly/discarded/x", blob, true, "").unwrap();
    assert!(gone(&dir, "edit-2-1").is_empty());
}

#[test]
fn gone_is_missing() {
    let (_fx, dir) = repo("gone");
    commit_files(&dir, &[("a.txt", "base\nedit-3-1\n")], "msg 3");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    git(&dir, &["reflog", "expire", "--expire=now", "--all"]);
    git(&dir, &["gc", "-q", "--prune=now"]);
    assert_eq!(missing(&dir, &set(&["edit-3-1"]), &set(&["msg 3"])), ["edit-3-1", "msg 3"]);
}

#[test]
fn crlf_marker_found() {
    let (_fx, dir) = repo("crlf");
    commit_files(&dir, &[("c.txt", "one\r\nedit-4-1\r\n")], "crlf");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    assert!(gone(&dir, "edit-4-1").is_empty());
    write(&dir, "d.txt", "x\r\nedit-4-2\r\n");
    assert!(gone(&dir, "edit-4-2").is_empty());
}

#[test]
fn summary_found_in_message() {
    let (_fx, dir) = repo("summary");
    commit_files(&dir, &[("a.txt", "changed\n")], "msg 5");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    assert!(missing(&dir, &BTreeSet::new(), &set(&["msg 5"])).is_empty());
    assert_eq!(missing(&dir, &BTreeSet::new(), &set(&["msg 6"])), ["msg 6"]);
}

#[test]
fn a_longer_number_is_not_the_marker() {
    let (_fx, dir) = repo("prefix");
    write(&dir, "a.txt", "edit-6-10\n");
    assert_eq!(gone(&dir, "edit-6-1"), ["edit-6-1"]);
}

#[test]
fn a_summary_is_the_whole_first_line() {
    let (_fx, dir) = repo("whole");
    commit_files(
        &dir,
        &[(
            "a.txt", "changed
",
        )],
        "msg 10

msg 1 in the body",
    );
    assert_eq!(missing(&dir, &BTreeSet::new(), &set(&["msg 1", "msg 10"])), ["msg 1"]);
}

#[test]
fn a_detached_head_is_walked_without_a_reflog() {
    let (_fx, dir) = repo("detached");
    git(&dir, &["checkout", "-q", "--detach"]);
    commit_files(
        &dir,
        &[(
            "m.txt",
            "edit-7-1
",
        )],
        "detached work",
    );
    // Only the commit HEAD points at holds the marker: out of the work tree and the index, on
    // no branch, and with no reflog left to name it.
    git(&dir, &["rm", "-q", "m.txt"]);
    std::fs::remove_dir_all(dir.join(".git").join("logs")).unwrap();
    assert!(gone(&dir, "edit-7-1").is_empty());
}

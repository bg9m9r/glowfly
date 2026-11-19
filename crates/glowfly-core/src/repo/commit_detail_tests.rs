//! `commit_message`, the whole message Copy > Message puts on the clipboard: multi-line bodies,
//! messages without a body, a Latin-1 commit, blank lines around a message and a missing commit.

use super::{commit_detail, commit_message};
use crate::repo::{LoadOptions, load};
use crate::testutil::{LATIN1_MSG, LATIN1_WHO, git, raw_commit, scratch_repo, write};
use git2::Oid;

fn head(dir: &std::path::Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn the_whole_message_keeps_its_line_breaks_without_a_trailing_newline() {
    let dir = scratch_repo("commit-message-body");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, ".git/COPY_MSG", "Add the page\n\nWhy:\n- one\n- two\n\nLast paragraph.\n");
    git(&dir, &["commit", "-q", "-F", ".git/COPY_MSG"]);
    let oid = head(&dir);
    let message = commit_message(&dir, oid).unwrap();
    assert_eq!(message, "Add the page\n\nWhy:\n- one\n- two\n\nLast paragraph.");
    assert_eq!(message, commit_detail(&dir, oid).unwrap().message);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_message_without_a_body_is_its_summary() {
    let dir = scratch_repo("commit-message-plain");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "Only a subject"]);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let message = commit_message(&dir, head(&dir)).unwrap();
    assert_eq!(message, "Only a subject");
    assert_eq!(message, snap.commits[0].summary.to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_latin1_message_is_decoded_as_the_details_show_it() {
    let dir = scratch_repo("commit-message-latin1");
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.treebuilder(None).unwrap().write().unwrap();
    let oid = raw_commit(&repo, tree, None, LATIN1_WHO, Some("ISO-8859-1"), LATIN1_MSG);
    assert_eq!(commit_message(&dir, oid).unwrap(), "Café au lait\n\nCrème brûlée for everyöne.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blank_lines_around_a_message_are_dropped_and_the_inside_kept() {
    let dir = scratch_repo("commit-message-edges");
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.treebuilder(None).unwrap().write().unwrap();
    let oid = raw_commit(&repo, tree, None, b"Test <test@example.com>", None, b"\n\nSubject\n\nBody  \n\n\n");
    assert_eq!(commit_message(&dir, oid).unwrap(), "Subject\n\nBody");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_commit_is_an_error() {
    let dir = scratch_repo("commit-message-gone");
    let oid = Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    assert!(commit_message(&dir, oid).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

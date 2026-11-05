//! The stash list read from the `refs/stash` reflog: what `git stash list` shows, an empty list
//! without a stash, a lossy message for bytes that aren't UTF-8, and an error for a ref that
//! can't be read.

use super::tests::{listed, rev};
use super::*;
use crate::testkit::git_cli;
use crate::testutil::{git, rewrite_reflog, scratch_repo, write};
use git2::Repository;

/// The stash `rev` names in `dir`.
fn oid_of(dir: &Path, r: &str) -> Oid {
    Oid::from_str(&rev(dir, r)).unwrap()
}

/// `(index, message, oid)` of each entry, to compare lists.
fn triples(list: &[StashInfo]) -> Vec<(usize, String, Oid)> {
    list.iter().map(|s| (s.index, s.message.clone(), s.oid)).collect()
}

/// A repository with a.txt committed.
fn committed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

#[test]
fn lists_what_git_stash_list_shows() {
    let dir = committed("stash-list-git");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["stash", "push", "-q", "-m", "one"]);
    write(&dir, "a.txt", "two\n");
    git(&dir, &["stash", "push", "-q"]);
    write(&dir, "a.txt", "three\n");
    git(&dir, &["stash", "push", "-q", "-m", "three"]);

    let ours = list(&dir).unwrap();
    let gits = listed(&dir);
    assert_eq!(ours.len(), 3);
    assert_eq!(gits.len(), 3, "{gits:?}");
    for (n, (s, (sha, text))) in ours.iter().zip(&gits).enumerate() {
        assert_eq!(s.index, n);
        assert_eq!(s.oid, oid_of(&dir, &format!("stash@{{{n}}}")));
        assert_eq!(s.oid.to_string(), *sha);
        assert_eq!(s.message, *text);
    }
    assert_eq!(ours[0].message, "On main: three");
    assert!(ours[1].message.starts_with("WIP on main: "), "{}", ours[1].message);
    assert_eq!(triples(&list_in(&Repository::open(&dir).unwrap()).unwrap()), triples(&ours));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_stash_is_an_empty_list() {
    let dir = committed("stash-list-empty");
    assert!(list(&dir).unwrap().is_empty());

    // A refs/stash without a reflog lists nothing, as libgit2's stash_foreach does.
    git_cli(&dir, &["update-ref", "--no-create-reflog", "refs/stash", &rev(&dir, "HEAD")]);
    let _ = std::fs::remove_file(dir.join(".git").join("logs").join("refs").join("stash"));
    assert!(Repository::open(&dir).unwrap().find_reference("refs/stash").is_ok());
    assert!(list(&dir).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_message_that_is_not_utf8_reads_lossily() {
    let dir = committed("stash-list-latin1");
    write(&dir, "a.txt", "first\n");
    git(&dir, &["stash", "push", "-q", "-m", "latin1-here"]);
    write(&dir, "a.txt", "second\n");
    git(&dir, &["stash", "push", "-q", "-m", "second"]);
    rewrite_reflog(&dir, "refs/stash", b"latin1-here", b"caf\xe9");

    let lossy = format!("On main: caf{}", char::REPLACEMENT_CHARACTER);
    assert_eq!(
        triples(&list(&dir).unwrap()),
        vec![(0, "On main: second".to_owned(), oid_of(&dir, "stash@{0}")), (1, lossy, oid_of(&dir, "stash@{1}"))]
    );
    // git itself still reads the list.
    git_cli(&dir, &["stash", "list"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_stash_ref_is_an_error() {
    let dir = committed("stash-list-corrupt");
    write(&dir, "a.txt", "changed\n");
    git(&dir, &["stash", "push", "-q"]);
    std::fs::write(dir.join(".git").join("refs").join("stash"), "not an id\n").unwrap();
    assert!(list(&dir).is_err(), "an unreadable ref is not an empty list");
    let _ = std::fs::remove_dir_all(&dir);
}

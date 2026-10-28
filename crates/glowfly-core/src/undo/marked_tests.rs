//! Undoing and redoing Mark resolved, Mark conflicted and a whole side taken, against real
//! merges: the conflict's stage entries come back exactly (modes and blobs, as `git ls-files -s`
//! lists them), so do the working bytes, redo does the change again, what an undo writes over
//! is kept for the journal, and a repository that moved on refuses with nothing written.

use super::*;
use crate::gitcli::run;
use crate::repo::{self, Side, mark_conflicted, mark_resolved, resolve_with_side};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// `file` mid-merge of `other`, its sides written as `content("base" | "ours" | "theirs")`
/// (`None`: deleted on that side).
pub(super) fn merging(name: &str, file: &str, content: impl Fn(&str) -> Option<Vec<u8>>) -> PathBuf {
    let dir = scratch_repo(name);
    let put = |side: &str| match content(side) {
        Some(b) => std::fs::write(dir.join(file), b).unwrap(),
        None => git(&dir, &["rm", "-q", file]),
    };
    put("base");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put("theirs");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put("ours");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

pub(super) fn text(name: &str) -> PathBuf {
    merging(name, "a.txt", |s| Some(format!("one\n{s}\n").into_bytes()))
}

pub(super) fn binary(name: &str) -> PathBuf {
    merging(name, "logo.bin", |s| Some(format!("\0\x01{s}\0").into_bytes()))
}

/// The index entries of `f`, as `git ls-files -s` lists them (mode, blob and stage).
pub(super) fn stages(dir: &Path, f: &str) -> String {
    let (ok, out) = run(dir, &args(&["ls-files", "-s", "--", f]));
    assert!(ok, "{out}");
    out
}

pub(super) fn bytes(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

pub(super) fn ids(dir: &Path, f: &str) -> repo::ConflictIds {
    repo::conflict_ids(&Repository::open(dir).unwrap(), f).expect("conflicted")
}

/// Plans and carries out an undo (or redo) of `a`; panics on a refusal.
pub(super) fn apply(dir: &Path, a: &Action, redo: bool) -> String {
    match plan(dir, a, redo).unwrap() {
        Plan::PutConflicts => put_conflicts(dir, a, redo).unwrap(),
        other => panic!("expected the files to be put back, got {other:?}"),
    }
}

pub(super) fn refusal(dir: &Path, a: &Action, redo: bool) -> String {
    match plan(dir, a, redo).unwrap() {
        Plan::Refuse(Refusal::Stale(m)) => m,
        other => panic!("expected a stale refusal, got {other:?}"),
    }
}

pub(super) fn kept_paths(dir: &Path) -> Vec<(String, Oid)> {
    kept::all_for_test(dir)
        .into_iter()
        .filter(|k| k.what == kept::KeptWhat::Conflict)
        .filter_map(|k| match k.item {
            kept::KeptItem::File { path, blob, .. } => Some((path, blob)),
            _ => None,
        })
        .collect()
}

pub(super) fn mark(dir: &Path, change: ConflictChange, paths: &[&str]) -> Action {
    let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
    let op = || match change {
        ConflictChange::MarkResolved => mark_resolved(dir, &paths),
        ConflictChange::MarkConflicted => mark_conflicted(dir, &paths),
        ConflictChange::Took(side) => resolve_with_side(dir, &paths[0], side, &ids(dir, &paths[0])),
    };
    conflicts_saving(dir, change, &paths, op).unwrap().expect("undoable")
}

#[test]
fn undoing_mark_resolved_restores_the_conflict_stages_exactly() {
    let dir = text("mk-resolved");
    let conflicted = stages(&dir, "a.txt");
    assert_eq!(conflicted.lines().count(), 3, "stages 1, 2 and 3: {conflicted}");
    let markers = bytes(&dir, "a.txt");
    write(&dir, "a.txt", "one\nmine\n");
    let edited = bytes(&dir, "a.txt");

    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    assert_eq!(a.describe(), "marking a.txt resolved");
    assert!(repo::conflict_ids(&Repository::open(&dir).unwrap(), "a.txt").is_none());
    let resolved = stages(&dir, "a.txt");

    assert_eq!(apply(&dir, &a, false), "Brought back the conflict in a.txt");
    assert_eq!(stages(&dir, "a.txt"), conflicted, "the same three stage entries");
    assert_eq!(bytes(&dir, "a.txt"), edited, "the working file is left as it was");
    assert!(repo::resolved_paths(&dir).unwrap().is_empty(), "the resolve-undo record went back into the index");
    assert_ne!(edited, markers);

    assert_eq!(apply(&dir, &a, true), "Marked a.txt resolved again");
    assert_eq!(stages(&dir, "a.txt"), resolved);
    assert_eq!(repo::resolved_paths(&dir).unwrap().len(), 1, "resolve-undo is recorded again");
    assert_eq!(apply(&dir, &a, false), "Brought back the conflict in a.txt");
    assert_eq!(stages(&dir, "a.txt"), conflicted);
    assert!(kept_paths(&dir).is_empty(), "nothing was written over");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undoing_mark_conflicted_puts_the_resolution_back() {
    let dir = text("mk-conflicted");
    write(&dir, "a.txt", "one\nresolved\n");
    mark_resolved(&dir, &["a.txt".to_owned()]).unwrap();
    write(&dir, "a.txt", "one\nresolved and edited\n");
    let resolution = stages(&dir, "a.txt");

    let a = mark(&dir, ConflictChange::MarkConflicted, &["a.txt"]);
    assert_eq!(a.describe(), "marking a.txt conflicted");
    let conflicted = stages(&dir, "a.txt");
    assert_eq!(conflicted.lines().count(), 3);
    let markers = bytes(&dir, "a.txt").unwrap();
    let edited = b"one\nresolved and edited\n".to_vec();
    let edited_blob = Oid::hash_object(git2::ObjectType::Blob, &edited).unwrap();
    assert!(kept_paths(&dir).contains(&("a.txt".to_owned(), edited_blob)), "the overwritten edit is kept");

    assert_eq!(apply(&dir, &a, false), "Put back the resolution of a.txt");
    assert_eq!(stages(&dir, "a.txt"), resolution);
    assert_eq!(bytes(&dir, "a.txt"), Some(edited.clone()));
    assert_eq!(repo::resolved_paths(&dir).unwrap().len(), 1, "it can be marked conflicted again");
    let markers_blob = Oid::hash_object(git2::ObjectType::Blob, &markers).unwrap();
    assert!(kept_paths(&dir).contains(&("a.txt".to_owned(), markers_blob)), "what the undo wrote over is kept");

    assert_eq!(apply(&dir, &a, true), "Brought back the conflict in a.txt");
    assert_eq!(stages(&dir, "a.txt"), conflicted);
    assert_eq!(bytes(&dir, "a.txt"), Some(markers));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undoing_a_whole_binary_side_brings_back_the_conflict_and_our_bytes() {
    let dir = binary("mk-binary");
    let conflicted = stages(&dir, "logo.bin");
    let ours = bytes(&dir, "logo.bin");
    assert_eq!(ours.as_deref(), Some(&b"\0\x01ours\0"[..]), "git leaves our version");

    let a = mark(&dir, ConflictChange::Took(Side::Theirs), &["logo.bin"]);
    assert_eq!(a.describe(), "taking their version of logo.bin");
    assert_eq!(bytes(&dir, "logo.bin").as_deref(), Some(&b"\0\x01theirs\0"[..]));
    let taken = stages(&dir, "logo.bin");

    assert_eq!(apply(&dir, &a, false), "Brought back the conflict in logo.bin");
    assert_eq!(stages(&dir, "logo.bin"), conflicted);
    assert_eq!(bytes(&dir, "logo.bin"), ours);

    assert_eq!(apply(&dir, &a, true), "Resolved logo.bin again with the same version");
    assert_eq!(stages(&dir, "logo.bin"), taken);
    assert_eq!(bytes(&dir, "logo.bin").as_deref(), Some(&b"\0\x01theirs\0"[..]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undoing_a_deletion_brings_back_a_modify_delete_conflict() {
    let dir = merging("mk-deleted", "gone.txt", |s| (s != "theirs").then(|| format!("kept {s}\n").into_bytes()));
    let conflicted = stages(&dir, "gone.txt");
    assert_eq!(conflicted.lines().count(), 2, "base and ours only: {conflicted}");
    let ours = bytes(&dir, "gone.txt");

    let a = mark(&dir, ConflictChange::Took(Side::Theirs), &["gone.txt"]);
    assert_eq!(a.describe(), "deletion of gone.txt to resolve the conflict");
    assert_eq!(bytes(&dir, "gone.txt"), None);

    apply(&dir, &a, false);
    assert_eq!(stages(&dir, "gone.txt"), conflicted, "git's resolve-undo can't bring this one back; the undo can");
    assert_eq!(bytes(&dir, "gone.txt"), ours);
    apply(&dir, &a, true);
    assert_eq!(stages(&dir, "gone.txt"), "");
    assert_eq!(bytes(&dir, "gone.txt"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undoing_a_submodule_side_restores_its_gitlink_stages() {
    let dir = text("mk-gitlink");
    let commit = |m: &str| {
        let repo = Repository::open(&dir).unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(None, &sig, &sig, m, &tree, &[]).unwrap().to_string()
    };
    let (base, ours, theirs) = (commit("base"), commit("ours"), commit("theirs"));
    let info = format!("0 {z}\tsub\n160000 {base} 1\tsub\n160000 {ours} 2\tsub\n160000 {theirs} 3\tsub\n", z = Oid::zero());
    let (ok, out) = crate::gitcli::run_with_input(&dir, &args(&["update-index", "--index-info"]), &info, &[]);
    assert!(ok, "{out}");
    let conflicted = stages(&dir, "sub");
    assert_eq!(conflicted.lines().count(), 3, "{conflicted}");

    let a = mark(&dir, ConflictChange::Took(Side::Ours), &["sub"]);
    assert_eq!(stages(&dir, "sub"), format!("160000 {ours} 0\tsub"));
    apply(&dir, &a, false);
    assert_eq!(stages(&dir, "sub"), conflicted);
    apply(&dir, &a, true);
    assert_eq!(stages(&dir, "sub"), format!("160000 {ours} 0\tsub"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn several_files_marked_resolved_come_back_together() {
    let dir = scratch_repo("mk-several");
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "base\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "theirs\n");
    }
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "ours\n");
    }
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0);
    let before = (stages(&dir, "a.txt"), stages(&dir, "b.txt"));

    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt", "b.txt"]);
    assert_eq!(a.describe(), "marking 2 files resolved");
    assert_eq!(apply(&dir, &a, false), "Brought back the conflicts in 2 files");
    assert_eq!((stages(&dir, "a.txt"), stages(&dir, "b.txt")), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_marked_is_no_entry() {
    let dir = text("mk-nothing");
    let none = conflicts_saving(&dir, ConflictChange::MarkResolved, &["a.txt".to_owned()], || Ok(())).unwrap();
    assert_eq!(none, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_that_moved_on_refuses_with_nothing_written() {
    // (a) The merge was aborted.
    let dir = text("mk-stale-abort");
    write(&dir, "a.txt", "one\nours\n");
    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    git(&dir, &["merge", "--abort"]);
    let m = refusal(&dir, &a, false);
    assert!(m.contains("The merge that was in progress during the marking a.txt resolved is over"), "{m}");
    assert!(m.ends_with("; removed from the undo history"), "{m}");
    assert_eq!(stages(&dir, "a.txt").lines().count(), 1, "nothing was put back");
    let _ = std::fs::remove_dir_all(&dir);

    // (b) The working file was edited after taking a side.
    let dir = binary("mk-stale-edit");
    let a = mark(&dir, ConflictChange::Took(Side::Theirs), &["logo.bin"]);
    write(&dir, "logo.bin", "by hand\n");
    assert!(refusal(&dir, &a, false).contains("logo.bin has changed since"));
    assert_eq!(bytes(&dir, "logo.bin").as_deref(), Some(&b"by hand\n"[..]));
    let _ = std::fs::remove_dir_all(&dir);

    // (c) The index changed: staged again by hand.
    let dir = text("mk-stale-index");
    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    write(&dir, "a.txt", "other\n");
    git(&dir, &["add", "a.txt"]);
    assert!(refusal(&dir, &a, false).contains("a.txt has changed in the index since"));
    let _ = std::fs::remove_dir_all(&dir);

    // (d) The merge was committed.
    let dir = text("mk-stale-head");
    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    git(&dir, &["commit", "-q", "-m", "merge"]);
    assert!(refusal(&dir, &a, false).contains("HEAD has moved since the marking a.txt resolved"));
    let _ = std::fs::remove_dir_all(&dir);
}

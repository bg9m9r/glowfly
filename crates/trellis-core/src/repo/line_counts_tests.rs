//! Line counts of commits and of the working tree, against real repositories.

use super::*;
use crate::repo::{commit_detail, status_at};
use crate::testutil::{git, scratch_repo, write};
use std::collections::BTreeSet;

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn n(added: usize, removed: usize) -> LineCount {
    LineCount { added, removed }
}

fn ten_lines(changed: Option<usize>) -> String {
    (0..10).map(|i| if Some(i) == changed { "changed\n".to_owned() } else { format!("line {i}\n") }).collect()
}

fn keys(c: &FileLineCounts) -> BTreeSet<&str> {
    c.files.keys().map(String::as_str).collect()
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn commit_counts_each_file() {
    let dir = scratch_repo("lines-commit");
    write(&dir, "a.txt", "one\ntwo\n");
    write(&dir, "c.txt", "x\ny\n");
    write(&dir, "d.txt", &ten_lines(None));
    commit_all(&dir, "base");
    write(&dir, "a.txt", "one\n2\nthree\n");
    write(&dir, "b.txt", "1\n2\n3\n");
    std::fs::remove_file(dir.join("c.txt")).unwrap();
    std::fs::remove_file(dir.join("d.txt")).unwrap();
    write(&dir, "e.txt", &ten_lines(Some(4)));
    let oid = commit_all(&dir, "edit");

    let c = commit_line_counts(&dir, oid).unwrap();
    assert_eq!(keys(&c), BTreeSet::from(["a.txt", "b.txt", "c.txt", "e.txt"]));
    assert_eq!(c.get("a.txt"), Some(n(2, 1)));
    assert_eq!(c.get("b.txt"), Some(n(3, 0)));
    assert_eq!(c.get("c.txt"), Some(n(0, 2)));
    assert_eq!(c.get("e.txt"), Some(n(1, 1)), "a rename counts only its changed line");
    assert_eq!(c.total, n(6, 4));
    assert_eq!(c.largest, 3);
    assert!(c.complete);
    cleanup(&dir);
}

#[test]
fn commit_counts_skip_binary() {
    let dir = scratch_repo("lines-binary");
    write(&dir, "a.txt", "one\ntwo\n");
    std::fs::write(dir.join("blob.bin"), b"\0\x01\x02binary\0").unwrap();
    let root = commit_all(&dir, "root");
    let c = commit_line_counts(&dir, root).unwrap();
    assert_eq!(keys(&c), BTreeSet::from(["a.txt"]), "the binary file has no entry");
    assert_eq!(c.total, n(2, 0), "a root commit counts every line as added");

    // A merge is counted against its first parent, as its file list is.
    git(&dir, &["checkout", "-q", "-b", "side"]);
    write(&dir, "side.txt", "s\n");
    commit_all(&dir, "side");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "main.txt", "m\nm\n");
    commit_all(&dir, "main");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge", "side"]);
    let merge = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let c = commit_line_counts(&dir, merge).unwrap();
    let listed: BTreeSet<String> = commit_detail(&dir, merge).unwrap().files.into_iter().map(|f| f.path).collect();
    assert_eq!(keys(&c), listed.iter().map(String::as_str).collect());
    assert_eq!(c.get("side.txt"), Some(n(1, 0)));
    assert_eq!(c.get("main.txt"), None);
    cleanup(&dir);
}

#[test]
fn keys_match_file_entries() {
    let dir = scratch_repo("lines-keys");
    write(&dir, "a.txt", "one\n");
    write(&dir, "old.txt", &ten_lines(None));
    commit_all(&dir, "base");
    std::fs::create_dir_all(dir.join("sub dir")).unwrap();
    write(&dir, "sub dir/new é.txt", "x\n");
    write(&dir, "a.txt", "two\n");
    let oid = commit_all(&dir, "second");
    let c = commit_line_counts(&dir, oid).unwrap();
    let rows: BTreeSet<String> = commit_detail(&dir, oid).unwrap().files.into_iter().map(|f| f.path).collect();
    assert_eq!(keys(&c), rows.iter().map(String::as_str).collect());

    git(&dir, &["mv", "old.txt", "renamed.txt"]);
    write(&dir, "a.txt", "three\n");
    write(&dir, "sub dir/untracked.txt", "u\n");
    let w = wip_line_counts(&dir).unwrap();
    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    let paths = |rows: Vec<crate::repo::FileEntry>| rows.into_iter().map(|f| f.path).collect::<BTreeSet<_>>();
    let (unstaged, staged) = (paths(unstaged), paths(staged));
    assert!(!w.staged.files.is_empty() && !w.unstaged.files.is_empty());
    assert!(keys(&w.staged).iter().all(|k| staged.contains(*k)), "{:?} vs {staged:?}", keys(&w.staged));
    assert!(keys(&w.unstaged).iter().all(|k| unstaged.contains(*k)), "{:?} vs {unstaged:?}", keys(&w.unstaged));
    cleanup(&dir);
}

#[test]
fn wip_counts_staged_and_unstaged() {
    let dir = scratch_repo("lines-wip");
    write(&dir, "a.txt", "one\ntwo\n");
    write(&dir, "gone.txt", "1\n2\n3\n");
    write(&dir, "old.txt", &ten_lines(None));
    write(&dir, ".gitignore", "*.log\n");
    commit_all(&dir, "base");

    write(&dir, "a.txt", "one\ntwo\nthree\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "a.txt", "one\ntwo\nfour\n");
    write(&dir, "new.txt", "1\n2\n3\n4\n");
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    git(&dir, &["mv", "old.txt", "moved.txt"]);
    write(&dir, "noise.log", "ignored\n");

    let w = wip_line_counts(&dir).unwrap();
    assert_eq!(keys(&w.staged), BTreeSet::from(["a.txt", "moved.txt"]), "a staged rename is keyed by its new path");
    assert_eq!(w.staged.get("a.txt"), Some(n(1, 0)));
    assert_eq!(w.staged.get("moved.txt"), Some(n(0, 0)));
    assert_eq!(keys(&w.unstaged), BTreeSet::from(["a.txt", "gone.txt", "new.txt"]), "ignored files are left out");
    assert_eq!(w.unstaged.get("a.txt"), Some(n(1, 1)));
    assert_eq!(w.unstaged.get("new.txt"), Some(n(4, 0)), "an untracked file counts all its lines as added");
    assert_eq!(w.unstaged.get("gone.txt"), Some(n(0, 3)));
    assert_eq!(w.total(), Some(n(6, 4)));
    assert_eq!(w.largest(), 4);
    cleanup(&dir);
}

#[test]
fn counts_stop_at_the_cap() {
    let dir = scratch_repo("lines-cap");
    for f in ["a.txt", "b.txt", "c.txt"] {
        write(&dir, f, "x\n");
    }
    let oid = commit_all(&dir, "three");
    let repo = Repository::open(&dir).unwrap();
    let diff = super::super::diff::commit_diff(&repo, oid, None).unwrap();
    let capped = counts_of(&repo, &diff, false, 2).unwrap();
    assert_eq!(capped.files.len(), 2);
    assert!(!capped.complete);
    assert!(counts_of(&repo, &diff, false, 3).unwrap().complete, "exactly the cap is complete");

    let w = WipLineCounts { staged: capped.clone(), unstaged: FileLineCounts { complete: true, ..Default::default() } };
    assert_eq!(w.total(), None, "one side is incomplete");
    drop(diff);
    drop(repo);
    cleanup(&dir);
}

#[test]
fn lfs_pointer_changes_have_no_entry() {
    // Pointer text committed without a filter: git stores it as it is, git-lfs or not.
    let ptr = |d: char| format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 12\n", d.to_string().repeat(64));
    let dir = scratch_repo("lines-lfs");
    write(&dir, "art.psd", &ptr('a'));
    write(&dir, "notes.txt", "plain\n");
    let oid = commit_all(&dir, "add");
    assert_eq!(keys(&commit_line_counts(&dir, oid).unwrap()), BTreeSet::from(["notes.txt"]));
    write(&dir, "art.psd", &ptr('b'));
    write(&dir, "notes.txt", "plain\nmore\n");
    let w = wip_line_counts(&dir).unwrap();
    assert_eq!(keys(&w.unstaged), BTreeSet::from(["notes.txt"]));
    cleanup(&dir);
}

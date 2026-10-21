//! Comparing a commit with the working tree: the changed files (staged, unstaged, untracked,
//! renamed; ignored ones left out) and a file's diff.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn entries(c: &Comparison) -> Vec<(&str, char)> {
    c.files.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

#[test]
fn lists_every_kind_of_change_since_the_commit() {
    let dir = scratch_repo("compare-workdir");
    write(&dir, ".gitignore", "*.log\n");
    write(&dir, "keep.txt", "keep\n");
    write(&dir, "mod.txt", "one\n");
    write(&dir, "gone.txt", "gone\n");
    write(&dir, "old.txt", &lines(30));
    write(&dir, "staged.txt", "s\n");
    let c1 = commit_all(&dir, "c1");
    assert!(compare_workdir(&dir, c1).unwrap().files.is_empty(), "a clean tree at HEAD matches it");

    write(&dir, "mod.txt", "one\ntwo\n");
    commit_all(&dir, "c2");
    write(&dir, "mod.txt", "one\ntwo\nthree\n");
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    write(&dir, "new.txt", "new\n");
    write(&dir, "debug.log", "ignored\n");
    write(&dir, "staged.txt", "s\nstaged\n");
    git(&dir, &["add", "staged.txt"]);
    git(&dir, &["mv", "old.txt", "renamed.txt"]);

    let c = compare_workdir(&dir, c1).unwrap();
    assert_eq!(
        entries(&c),
        [("gone.txt", 'D'), ("mod.txt", 'M'), ("new.txt", 'A'), ("renamed.txt", 'R'), ("staged.txt", 'M')],
        "{:?}",
        c.files
    );
    assert!(!c.in_workdir("gone.txt") && c.in_workdir("new.txt") && c.in_workdir("renamed.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_diff_runs_from_the_commit_to_the_working_file() {
    let dir = scratch_repo("compare-workdir-diff");
    write(&dir, "f.txt", "a\n");
    write(&dir, "old.txt", &lines(30));
    let c1 = commit_all(&dir, "c1");
    write(&dir, "f.txt", "a\nb\n");
    commit_all(&dir, "c2");
    write(&dir, "f.txt", "a\nb\nc\n");
    write(&dir, "u.txt", "untracked\n");
    git(&dir, &["mv", "old.txt", "renamed.txt"]);
    write(&dir, "renamed.txt", &format!("{}line 31\n", lines(30)));

    let changed = |path: &str| -> Vec<String> {
        let diff = load_diff(&dir, &DiffTarget::ToWorkdir(c1, path.into())).unwrap();
        diff.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| matches!(l.origin, '+' | '-'))
            .map(|l| format!("{}{}", l.origin, l.text))
            .collect()
    };
    assert_eq!(changed("f.txt"), ["+b", "+c"], "both the commit after and the working edit");
    assert_eq!(changed("u.txt"), ["+untracked"], "an untracked file shows in full as added");
    assert_eq!(changed("renamed.txt"), ["+line 31"], "the rename is found by its new path");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_other_way_round_lists_from_the_working_tree_to_the_commit() {
    let dir = scratch_repo("compare-from-workdir");
    write(&dir, "mod.txt", "one\n");
    write(&dir, "gone.txt", "gone\n");
    let c1 = commit_all(&dir, "c1");
    write(&dir, "mod.txt", "one\ntwo\n");
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    write(&dir, "new.txt", "new\n");
    write(&dir, "staged.txt", "staged\n");
    git(&dir, &["add", "staged.txt"]);

    let c = compare_from_workdir(&dir, c1).unwrap();
    assert_eq!(entries(&c), [("gone.txt", 'A'), ("mod.txt", 'M'), ("new.txt", 'D'), ("staged.txt", 'D')], "{:?}", c.files);
    assert!(!c.in_workdir("gone.txt") && c.in_workdir("new.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each line of `target`'s diff as (origin, old line, new line, text).
fn diff_lines(dir: &Path, target: &DiffTarget) -> Vec<(char, Option<u32>, Option<u32>, String)> {
    let diff = load_diff(dir, target).unwrap();
    diff.hunks.iter().flat_map(|h| &h.lines).map(|l| (l.origin, l.old, l.new, l.text.clone())).collect()
}

#[test]
fn a_file_diff_from_the_working_tree_is_the_reverse() {
    let dir = scratch_repo("compare-from-workdir-diff");
    write(&dir, "f.txt", &lines(12));
    let c1 = commit_all(&dir, "c1");
    write(&dir, "f.txt", &lines(12).replace("line 2\n", "line two\n").replace("line 10\n", "line 10\nline 10b\n"));
    write(&dir, "u.txt", "untracked\nfile\n");

    let forward = diff_lines(&dir, &DiffTarget::ToWorkdir(c1, "f.txt".into()));
    let swap = |o: char| match o {
        '+' => '-',
        '-' => '+',
        o => o,
    };
    // A changed line pair comes out removed first, so compare each run of changes as a set.
    let mut reversed: Vec<_> = forward.iter().map(|(o, old, new, t)| (swap(*o), *new, *old, t.clone())).collect();
    let mut back = diff_lines(&dir, &DiffTarget::FromWorkdir(c1, "f.txt".into()));
    assert_eq!(back.len(), reversed.len(), "{back:?}");
    reversed.sort_by_key(|l| (l.1.or(l.2), l.0));
    back.sort_by_key(|l| (l.1.or(l.2), l.0));
    assert_eq!(back, reversed);

    let untracked = diff_lines(&dir, &DiffTarget::FromWorkdir(c1, "u.txt".into()));
    assert!(!untracked.is_empty() && untracked.iter().all(|l| l.0 == '-'), "{untracked:?}");

    let s = diff_sides(&dir, &DiffTarget::FromWorkdir(c1, "f.txt".into())).unwrap();
    assert_eq!(s.old, SideSource::Working("f.txt".into()));
    assert!(matches!(&s.new, SideSource::Blob { path, .. } if path == "f.txt"), "{:?}", s.new);
    assert_eq!((s.old_label.as_str(), s.new_label.as_str()), ("working tree", &c1.to_string()[..7]));
    let _ = std::fs::remove_dir_all(&dir);
}

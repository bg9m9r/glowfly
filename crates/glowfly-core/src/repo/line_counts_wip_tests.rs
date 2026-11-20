//! The working tree's line counts read for a status read's paths (`wip_line_counts_of`) against
//! the whole-tree scan (`wip_line_counts`), on real repositories.

use super::*;
use crate::repo::{FileEntry, status_at};
use crate::testutil::{git, scratch_repo, with_submodule, write};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn n(added: usize, removed: usize) -> LineCount {
    LineCount { added, removed }
}

fn keys(c: &FileLineCounts) -> BTreeSet<&str> {
    c.files.keys().map(String::as_str).collect()
}

fn ten_lines(changed: Option<usize>) -> String {
    (0..10).map(|i| if Some(i) == changed { "changed\n".to_owned() } else { format!("line {i}\n") }).collect()
}

fn pointer(digit: char) -> String {
    format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 12\n", digit.to_string().repeat(64))
}

fn put(dir: &Path, file: &str, text: &str) {
    if let Some(parent) = dir.join(file).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    write(dir, file, text);
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// Removes a `with_submodule` fixture: the parent and the sibling repository it was added from.
fn clean_submodule_fixture(parent: &Path) {
    let lib = parent.with_file_name(format!("{}-lib", parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(parent);
    let _ = std::fs::remove_dir_all(lib);
}

/// A repository with one of everything a count meets, left uncommitted.
fn changed_tree(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    put(&dir, ".gitignore", "*.log\n");
    put(&dir, "edit.txt", &ten_lines(None));
    put(&dir, "both.txt", &ten_lines(None));
    put(&dir, "gone-staged.txt", "1\n2\n");
    put(&dir, "gone-unstaged.txt", "1\n2\n3\n");
    put(&dir, "old.txt", &ten_lines(Some(1)));
    put(&dir, "moved-from.txt", &ten_lines(Some(2)));
    put(&dir, "bin.dat", "a\0b\n");
    put(&dir, "art.psd", &pointer('a'));
    put(&dir, "sub dir/kept.txt", "kept\n");
    commit_all(&dir, "base");

    put(&dir, "edit.txt", &ten_lines(Some(3)));
    put(&dir, "both.txt", &ten_lines(Some(4)));
    git(&dir, &["add", "both.txt"]);
    put(&dir, "both.txt", &ten_lines(Some(5)));
    git(&dir, &["rm", "-q", "gone-staged.txt"]);
    std::fs::remove_file(dir.join("gone-unstaged.txt")).unwrap();
    git(&dir, &["mv", "old.txt", "renamed.txt"]);
    git(&dir, &["mv", "moved-from.txt", "moved-to.txt"]);
    put(&dir, "moved-to.txt", &ten_lines(Some(6)));
    git(&dir, &["add", "moved-to.txt"]);
    put(&dir, "new/top.txt", "1\n2\n");
    put(&dir, "new/deep/er/u.txt", "1\n2\n3\n");
    put(&dir, "bin.dat", "a\0c\n");
    put(&dir, "new.bin", "\0\0\0");
    put(&dir, "ita.txt", "i\nt\na\n");
    git(&dir, &["add", "-N", "ita.txt"]);
    put(&dir, "sub dir/new \u{e9}.txt", "e\n");
    put(&dir, "art.psd", &pointer('b'));
    put(&dir, "noise.log", "ignored\n");
    dir
}

/// The counts for the rows of both ways of reading the status equal the whole-tree counts.
fn assert_scoped_matches(dir: &Path) -> WipLineCounts {
    let whole = wip_line_counts(dir).unwrap();
    for git_status in [true, false] {
        let (unstaged, staged, _) = status_at(dir, git_status).unwrap();
        let scoped = wip_line_counts_of(dir, &WipPaths::of(&staged, &unstaged)).unwrap();
        assert_eq!(scoped, whole, "git_status {git_status}");
    }
    whole
}

#[test]
fn scoped_counts_match_the_whole_tree() {
    let dir = changed_tree("wipscope-all");
    let w = assert_scoped_matches(&dir);
    assert_eq!(w.staged.get("moved-to.txt"), Some(n(2, 2)), "an edited rename is counted under its new path");
    assert_eq!(w.staged.get("renamed.txt"), Some(n(0, 0)));
    assert_eq!(w.unstaged.get("new/deep/er/u.txt"), Some(n(3, 0)), "nested untracked folders count as added");
    assert_eq!(w.unstaged.get("sub dir/new \u{e9}.txt"), Some(n(1, 0)));
    assert!(w.unstaged.get("ita.txt").is_some());
    assert!(!keys(&w.unstaged).contains("noise.log") && !keys(&w.unstaged).contains("new.bin") && !keys(&w.unstaged).contains("art.psd"));
    assert!(!keys(&w.unstaged).contains("bin.dat") && !keys(&w.staged).contains("art.psd"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn submodule_rows_match_the_whole_tree() {
    let (parent, sub) = with_submodule("wipscope-sub");
    put(&sub, "lib.txt", "lib\nmore\n");
    commit_all(&sub, "moved on");
    put(&sub, "loose.txt", "loose\n");
    put(&parent, "a.txt", "one\ntwo\n");
    let (unstaged, ..) = status_at(&parent, false).unwrap();
    assert!(unstaged.iter().any(|f| f.path == "libs/lib" && f.status == 'M'), "{unstaged:?}");
    let w = assert_scoped_matches(&parent);
    assert_eq!(w.unstaged.get("a.txt"), Some(n(1, 0)));
    assert!(!w.unstaged.files.contains_key("libs/lib") && !w.staged.files.contains_key("libs/lib"));
    clean_submodule_fixture(&parent);
}

#[test]
fn a_conflict_matches_the_whole_tree() {
    let dir = scratch_repo("wipscope-conflict");
    put(&dir, "a.txt", "base\n");
    put(&dir, "b.txt", "b\n");
    commit_all(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put(&dir, "a.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "a.txt", "ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "other"]));
    assert!(!ok, "the merge should stop for a conflict");
    put(&dir, "b.txt", "b\nmore\n");
    let (unstaged, ..) = status_at(&dir, false).unwrap();
    assert!(unstaged.iter().any(|f| f.path == "a.txt" && f.status == '!'), "{unstaged:?}");
    let w = assert_scoped_matches(&dir);
    assert!(!w.unstaged.files.contains_key("a.txt") && !w.staged.files.contains_key("a.txt"));
    assert_eq!(w.unstaged.get("b.txt"), Some(n(1, 0)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pathspecs_are_literal() {
    let dir = scratch_repo("wipscope-literal");
    put(&dir, "[ab].txt", "x\n");
    put(&dir, "a.txt", "x\n");
    commit_all(&dir, "base");
    put(&dir, "[ab].txt", "x\ny\n");
    put(&dir, "a.txt", "x\ny\n");
    let only = WipPaths::of(&[], &[FileEntry::new("[ab].txt", 'M')]);
    let w = wip_line_counts_of(&dir, &only).unwrap();
    assert_eq!(keys(&w.unstaged), BTreeSet::from(["[ab].txt"]));
    assert!(w.staged.files.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untracked_folders_are_walked_only_under_listed_paths() {
    let dir = scratch_repo("wipscope-untracked");
    put(&dir, "a.txt", "x\n");
    commit_all(&dir, "base");
    put(&dir, "new/a/x.txt", "x\n");
    put(&dir, "other/y.txt", "y\n");
    let only = WipPaths::of(&[], &[FileEntry::new("new/a/x.txt", '?')]);
    let w = wip_line_counts_of(&dir, &only).unwrap();
    assert_eq!(keys(&w.unstaged), BTreeSet::from(["new/a/x.txt"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_rename_is_paired_through_its_old_path() {
    let dir = changed_tree("wipscope-rename");
    let (_, staged, _) = status_at(&dir, false).unwrap();
    let moved = staged.iter().find(|f| f.path == "moved-to.txt").unwrap();
    assert_eq!((moved.status, moved.renamed_from.as_deref()), ('R', Some("moved-from.txt")));
    let w = wip_line_counts_of(&dir, &WipPaths::of(&staged, &[])).unwrap();
    assert_eq!(w.staged.get("moved-to.txt"), Some(n(2, 2)));

    // Without the old path the new one has nothing to pair with.
    let alone = WipPaths { paths: vec![b"moved-to.txt".to_vec()], ..Default::default() };
    let w = wip_line_counts_of(&dir, &alone).unwrap();
    assert_eq!(w.staged.get("moved-to.txt"), Some(n(10, 0)), "counted as all added");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn long_lists_count_their_first_rows() {
    let dir = scratch_repo("wipscope-long");
    put(&dir, "base.txt", "x\n");
    commit_all(&dir, "base");
    for f in ["a.txt", "b.txt", "c.txt"] {
        put(&dir, f, "x\ny\n");
    }
    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    let whole = wip_line_counts(&dir).unwrap();
    let w = wip_line_counts_of(&dir, &WipPaths::of_capped(&staged, &unstaged, 2)).unwrap();
    assert_eq!(w.unstaged.files.len(), 2);
    assert!(!w.unstaged.complete && w.staged.complete);
    assert_eq!(w.total(), None);
    for (path, count) in &w.unstaged.files {
        assert_eq!(whole.unstaged.files.get(path), Some(count));
    }
    assert_eq!(keys(&w.unstaged), BTreeSet::from(["a.txt", "b.txt"]), "the rows at the top of the list");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_scope_counts_nothing() {
    let dir = changed_tree("wipscope-empty");
    let w = wip_line_counts_of(&dir, &WipPaths::default()).unwrap();
    assert!(w.staged.complete && w.unstaged.complete);
    assert!(w.staged.files.is_empty() && w.unstaged.files.is_empty());
    assert_eq!(w.total(), Some(LineCount::default()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_non_utf8_name_is_scoped_by_its_bytes() {
    use std::os::unix::ffi::OsStrExt;
    let dir = scratch_repo("wipscope-bytes");
    put(&dir, "a.txt", "x\n");
    commit_all(&dir, "base");
    let name = std::ffi::OsStr::from_bytes(b"odd-\xFF.txt");
    // Some file systems (macOS's) refuse such a name.
    if std::fs::write(dir.join(name), "1\n2\n").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    let row = unstaged.iter().find(|f| matches!(f.hazard, Some(crate::repo::Hazard::NotUtf8(_)))).expect("a row for the odd name");
    let key = crate::repo::shown_path(b"odd-\xFF.txt");
    assert_eq!(row.path, key);
    let scoped = wip_line_counts_of(&dir, &WipPaths::of(&staged, &unstaged)).unwrap();
    assert_eq!(scoped, wip_line_counts(&dir).unwrap());
    assert_eq!(scoped.unstaged.get(&key), Some(n(2, 0)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn git_and_libgit2_rows_match_with_a_submodule_an_intent_to_add_file_and_a_binary() {
    let (parent, sub) = with_submodule("wipscope-rows");
    put(&sub, "lib.txt", "lib\nmore\n");
    commit_all(&sub, "moved on");
    put(&parent, "ita.txt", "i\n");
    git(&parent, &["add", "-N", "ita.txt"]);
    put(&parent, "new.bin", "\0\0");
    put(&parent, "art.psd", &pointer('c'));
    let (u_git, s_git, _) = status_at(&parent, true).unwrap();
    let (u_lib, s_lib, _) = status_at(&parent, false).unwrap();
    assert_eq!((&u_git, &s_git), (&u_lib, &s_lib));
    assert!(u_git.iter().any(|f| f.path == "art.psd" && f.lfs), "{u_git:?}");
    clean_submodule_fixture(&parent);
}

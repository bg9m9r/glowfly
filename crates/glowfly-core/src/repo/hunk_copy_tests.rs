//! Copy hunk as patch against real repositories: the patch `hunk_patch` writes is checked with
//! `git apply` (the git CLI) on the diff's old side, for plain, CRLF, Latin-1, new, renamed,
//! executable and whitespace-hidden files.

use crate::repo::{DiffTarget, LoadDiffOptions, hunk_patch, load_diff_with};
use crate::testkit::git_cli;
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

/// A repository with `f` committed as `base` and changed to `now` in the working tree.
fn edited(name: &str, f: &str, base: &[u8], now: &[u8]) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::write(dir.join(f), base).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    std::fs::write(dir.join(f), now).unwrap();
    dir
}

/// Hunk 0 of `target` as a patch, the diff loaded with `ignore_whitespace` as the viewer would.
fn patch_of(dir: &Path, target: &DiffTarget, ignore_whitespace: bool) -> Vec<u8> {
    let d = load_diff_with(dir, target, LoadDiffOptions { ignore_whitespace, ..Default::default() }).unwrap();
    hunk_patch(dir, target, 0, &d.hunks[0], ignore_whitespace).unwrap()
}

/// Checks `patch` with `git apply --check`, then applies it, in `dir`'s working tree.
fn apply(dir: &Path, patch: &[u8]) {
    // Outside the working tree, so the patch file isn't a change itself.
    let file = dir.with_extension("patch");
    std::fs::write(&file, patch).unwrap();
    let file = file.to_string_lossy().into_owned();
    git_cli(dir, &["apply", "--check", &file]);
    git_cli(dir, &["apply", &file]);
    let _ = std::fs::remove_file(&file);
}

fn unstaged(path: &str) -> DiffTarget {
    DiffTarget::Unstaged(path.into())
}

#[test]
fn a_hunk_copied_as_a_patch_applies_to_the_old_file() {
    let (base, now) = ("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n", "1\n2\n3\n4\nfive\n6\n6a\n6b\n7\n8\n9\n10\n");
    let dir = edited("hunk-copy", "f.txt", base.as_bytes(), now.as_bytes());
    let patch = String::from_utf8(patch_of(&dir, &unstaged("f.txt"), false)).unwrap();
    assert!(patch.starts_with("diff --git a/f.txt b/f.txt\n"), "{patch}");
    assert!(patch.contains("\n 4\n-5\n+five\n 6\n+6a\n+6b\n 7\n"), "the hunk with its context: {patch}");
    // Checked against the file as committed, then applied: the edit comes back.
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, patch.as_bytes());
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), now.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_crlf_files_hunk_keeps_its_line_endings() {
    let dir = edited("hunk-copy-crlf", "f.txt", b"a\r\nb\r\nc\r\n", b"a\r\nB\r\nc\r\n");
    let patch = patch_of(&dir, &unstaged("f.txt"), false);
    assert!(patch.windows(5).any(|w| w == b"\n-b\r\n"), "{}", String::from_utf8_lossy(&patch));
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, &patch);
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"a\r\nB\r\nc\r\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_latin1_files_hunk_keeps_its_bytes() {
    let dir = edited("hunk-copy-latin1", "f.txt", b"caf\xe9\nb\nc\n", b"caf\xe9\nB\nc\n");
    let patch = patch_of(&dir, &unstaged("f.txt"), false);
    assert!(String::from_utf8(patch.clone()).is_err(), "the file's own bytes, not U+FFFD");
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, &patch);
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"caf\xe9\nB\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_files_hunk_copied_as_a_patch_creates_it() {
    let dir = edited("hunk-copy-new", "base.txt", b"x\n", b"x\n");
    write(&dir, "new.txt", "one\ntwo");
    git(&dir, &["add", "-N", "new.txt"]);
    let patch = String::from_utf8(patch_of(&dir, &unstaged("new.txt"), false)).unwrap();
    assert!(patch.contains("new file mode 100644\n"), "{patch}");
    assert!(patch.contains("\\ No newline at end of file\n"), "{patch}");
    std::fs::remove_file(dir.join("new.txt")).unwrap();
    git(&dir, &["reset", "-q"]);
    apply(&dir, patch.as_bytes());
    assert_eq!(std::fs::read(dir.join("new.txt")).unwrap(), b"one\ntwo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_renamed_files_hunk_names_both_paths() {
    let base = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    let dir = edited("hunk-copy-rename", "old.txt", base.as_bytes(), base.as_bytes());
    git(&dir, &["mv", "old.txt", "new.txt"]);
    write(&dir, "new.txt", &base.replace("5\n", "five\n"));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "rename"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().peel_to_commit().unwrap().id();
    let patch = String::from_utf8(patch_of(&dir, &DiffTarget::Commit(head, "new.txt".into()), false)).unwrap();
    assert!(patch.starts_with("diff --git a/old.txt b/new.txt\nrename from old.txt\nrename to new.txt\n"), "{patch}");
    assert!(patch.contains("\n--- a/old.txt\n+++ b/new.txt\n"), "{patch}");
    // On the commit's parent, where the file still has its old name.
    git_cli(&dir, &["checkout", "-q", "HEAD~1"]);
    apply(&dir, patch.as_bytes());
    assert!(!dir.join("old.txt").exists());
    assert_eq!(std::fs::read_to_string(dir.join("new.txt")).unwrap(), base.replace("5\n", "five\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_executable_files_hunk_keeps_its_mode() {
    let dir = edited("hunk-copy-mode", "run.sh", b"echo a\necho b\n", b"echo a\necho b\n");
    git_cli(&dir, &["update-index", "--chmod=+x", "run.sh"]);
    git(&dir, &["commit", "-m", "executable"]);
    write(&dir, "run.sh", "echo a\necho B\n");
    git(&dir, &["add", "run.sh"]);
    let patch = String::from_utf8(patch_of(&dir, &DiffTarget::Staged("run.sh".into()), false)).unwrap();
    assert!(patch.contains("\nindex 0000000..0000000 100755\n"), "{patch}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_whitespace_hidden_the_hunk_applies_to_the_old_file() {
    // The indent of `b` changes too, hidden; the change to `d` is shown.
    let dir = edited("hunk-copy-ws", "f.txt", b"a\n  b\nc\nd\ne\n", b"a\n    b\nc\nD\ne\n");
    let patch = String::from_utf8(patch_of(&dir, &unstaged("f.txt"), true)).unwrap();
    assert!(patch.contains("\n   b\n c\n-d\n+D\n e\n"), "the old side's context: {patch}");
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, patch.as_bytes());
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"a\n  b\nc\nD\ne\n", "only the change shown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hunk_that_changed_since_it_was_shown_is_refused() {
    let dir = edited("hunk-copy-stale", "f.txt", b"a\nb\nc\n", b"a\nB\nc\n");
    let target = unstaged("f.txt");
    let shown = load_diff_with(&dir, &target, LoadDiffOptions::default()).unwrap();
    write(&dir, "f.txt", "a\nX\nc\n");
    let r = hunk_patch(&dir, &target, 0, &shown.hunks[0], false);
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{:?}", r.ok().map(|b| String::from_utf8_lossy(&b).into_owned()));
    assert!(hunk_patch(&dir, &target, 1, &shown.hunks[0], false).is_err(), "no such hunk");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_first_edit_to_an_empty_file_patches_the_file_that_exists() {
    let dir = edited("hunk-copy-empty", "f.txt", b"", b"one\ntwo\n");
    let patch = String::from_utf8(patch_of(&dir, &unstaged("f.txt"), false)).unwrap();
    assert!(patch.contains("\n--- a/f.txt\n+++ b/f.txt\n@@ -0,0 +1,2 @@\n"), "not a new file: {patch}");
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, patch.as_bytes());
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"one\ntwo\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_files_hunk_deletes_it() {
    let dir = edited("hunk-copy-deleted", "f.txt", b"a\nb\n", b"a\nb\n");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    let patch = String::from_utf8(patch_of(&dir, &unstaged("f.txt"), false)).unwrap();
    assert!(patch.contains("deleted file mode 100644\n--- a/f.txt\n+++ /dev/null\n"), "{patch}");
    git(&dir, &["checkout", "--", "f.txt"]);
    apply(&dir, patch.as_bytes());
    assert!(!dir.join("f.txt").exists(), "deleted, not emptied");
    let _ = std::fs::remove_dir_all(&dir);
}

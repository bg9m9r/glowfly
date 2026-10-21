//! Reading merge-tree's output, and predicting on fixture repositories: conflicts, clean
//! merges, branches that contain each other, odd paths, unrelated histories, and that nothing
//! in the repository changes.

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

const TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn out(code: i32, stdout: &str, stderr: &str) -> GitOutput {
    GitOutput { ok: code == 0, code: Some(code), stdout: stdout.into(), stderr: stderr.into(), ..GitOutput::default() }
}

#[test]
fn clean_merge() {
    assert_eq!(parse_merge_tree(&out(0, &format!("{TREE}\0"), "")).unwrap(), Prediction::Clean);
}

#[test]
fn conflicted_paths_in_order_once_each() {
    let o = out(1, &format!("{TREE}\0a.txt\0dir/b c.txt\0a.txt\0"), "");
    assert_eq!(parse_merge_tree(&o).unwrap(), Prediction::Conflicts(vec!["a.txt".into(), "dir/b c.txt".into()]));
    let sha256 = "a".repeat(64);
    assert_eq!(parse_merge_tree(&out(1, &format!("{sha256}\0x\0"), "")).unwrap(), Prediction::Conflicts(vec!["x".into()]));
}

#[test]
fn exit_one_without_a_tree_is_an_error() {
    let o = out(1, "", "merge-tree: x - not something we can merge\n");
    let e = parse_merge_tree(&o).unwrap_err();
    assert!(e.to_string().contains("not something we can merge"), "{e}");
}

#[test]
fn other_failures_carry_git_text() {
    let e = parse_merge_tree(&out(128, "", "fatal: refusing to merge unrelated histories\n")).unwrap_err();
    assert_eq!(e.to_string(), "fatal: refusing to merge unrelated histories");
    assert_eq!(parse_merge_tree(&out(2, "", "")).unwrap_err().to_string(), "git merge-tree failed");
}

#[test]
fn usage_text_means_git_is_too_old() {
    let e = parse_merge_tree(&out(129, "", "usage: git merge-tree <base-tree> <branch1> <branch2>\n")).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)), "{e:?}");
    assert!(e.to_string().contains("2.38"), "{e}");
}

#[test]
fn timeout_is_an_error() {
    let o = GitOutput { timed_out: true, ..GitOutput::default() };
    assert!(parse_merge_tree(&o).unwrap_err().to_string().contains("too long"));
}

#[test]
fn too_old_names_both_versions() {
    let e = too_old(Some((2, 30, 1)));
    assert_eq!(e.to_string(), "Predicting conflicts needs git 2.38 or newer; this is git 2.30.1.");
}

/// A repository whose `main` has one commit with f.txt (three lines) and other.txt, and a
/// `feature` branch at that commit; `main` is checked out.
fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", "a\nb\nc\n");
    write(&dir, "other.txt", "other\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["branch", "feature"]);
    dir
}

fn commit_on(dir: &Path, branch: &str, file: &str, text: &str) {
    git(dir, &["checkout", "-q", branch]);
    if let Some(parent) = Path::new(file).parent() {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", &format!("{branch} edits {file}")]);
}

fn oid(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().id()
}

fn predict_branches(dir: &Path, ours: &str, theirs: &str) -> Result<Prediction> {
    predict(dir, oid(dir, ours), oid(dir, theirs))
}

#[test]
fn same_line_on_both_sides_conflicts() {
    let dir = base("cc-same-line");
    commit_on(&dir, "main", "f.txt", "a\nmain\nc\n");
    commit_on(&dir, "feature", "f.txt", "a\nfeature\nc\n");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::Conflicts(vec!["f.txt".into()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn different_files_merge_cleanly() {
    let dir = base("cc-clean");
    commit_on(&dir, "main", "f.txt", "a\nmain\nc\n");
    commit_on(&dir, "feature", "other.txt", "changed\n");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contained_branches_skip_merge_tree() {
    let dir = base("cc-contained");
    commit_on(&dir, "feature", "f.txt", "a\nfeature\nc\n");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::UpToDate);
    assert_eq!(predict_branches(&dir, "feature", "feature").unwrap(), Prediction::UpToDate);
    assert_eq!(predict_branches(&dir, "main", "feature").unwrap(), Prediction::FastForward);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn modify_delete_is_listed() {
    let dir = base("cc-modify-delete");
    git(&dir, &["rm", "-q", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "drop f"]);
    commit_on(&dir, "feature", "f.txt", "a\nfeature\nc\n");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::Conflicts(vec!["f.txt".into()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn odd_paths_come_back_exactly() {
    let dir = base("cc-odd-path");
    let path = "dir/b c \u{fc}.txt";
    commit_on(&dir, "main", path, "main\n");
    commit_on(&dir, "feature", path, "feature\n");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::Conflicts(vec![path.into()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unrelated_histories_are_an_error() {
    let dir = base("cc-orphan");
    git(&dir, &["checkout", "-q", "--orphan", "other"]);
    git(&dir, &["rm", "-rq", "--cached", "."]);
    write(&dir, "new.txt", "new\n");
    git(&dir, &["add", "new.txt"]);
    git(&dir, &["commit", "-q", "-m", "orphan"]);
    let e = predict_branches(&dir, "main", "other").unwrap_err();
    assert!(e.to_string().contains("share no history"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_in_the_repository_changes() {
    let dir = base("cc-untouched");
    commit_on(&dir, "main", "f.txt", "a\nmain\nc\n");
    commit_on(&dir, "feature", "f.txt", "a\nfeature\nc\n");
    write(&dir, "other.txt", "dirty\n");
    write(&dir, "staged.txt", "staged\n");
    git(&dir, &["add", "staged.txt"]);
    let state = || {
        let read = |a: &[&str]| crate::gitcli::read(&dir, &crate::gitcli::args(a)).stdout;
        let status = read(&["status", "--porcelain=v1"]);
        let refs = read(&["for-each-ref"]);
        let index = std::fs::read(dir.join(".git/index")).unwrap();
        let head = std::fs::read(dir.join(".git/HEAD")).unwrap();
        let files = (std::fs::read(dir.join("f.txt")).unwrap(), std::fs::read(dir.join("other.txt")).unwrap());
        (status, refs, index, head, files)
    };
    let before = state();
    assert!(!before.0.is_empty(), "the fixture is dirty");
    assert_eq!(predict_branches(&dir, "feature", "main").unwrap(), Prediction::Conflicts(vec!["f.txt".into()]));
    assert_eq!(state(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

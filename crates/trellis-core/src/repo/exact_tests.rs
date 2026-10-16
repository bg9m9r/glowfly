//! Index edits reach exactly the path asked for: never a glob match, never a path that differs
//! only in case. The case twins are index entries made with `update-index --cacheinfo`, so no
//! file system has to hold both and the tests run on every OS.

use super::bytes_tests::{blob, git_in};
use super::*;
use crate::testutil::*;
use std::path::Path;

/// The blob id `path` has in the index (stage 0).
fn staged_id(dir: &Path, path: &str) -> String {
    git_in(dir, &["rev-parse", &format!(":0:{path}")], b"")
}

fn staged_paths(dir: &Path) -> Vec<String> {
    status_at(dir, false).unwrap().1.into_iter().map(|f| f.path).collect()
}

#[test]
fn unstaging_a_name_with_glob_characters_unstages_only_that_file() {
    let dir = scratch_repo("exact-unstage-glob");
    for f in ["[id].tsx", "i.tsx", "d.tsx"] {
        write(&dir, f, "old\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    for f in ["[id].tsx", "i.tsx", "d.tsx"] {
        write(&dir, f, "new\n");
    }
    write(&dir, "n.tsx", "added\n");
    git(&dir, &["add", "."]);
    unstage_paths(&dir, &["[id].tsx".into()]).unwrap();
    assert_eq!(staged_paths(&dir), ["d.tsx", "i.tsx", "n.tsx"]);
    // A staged new file leaves the index; the others stay.
    unstage_paths(&dir, &["n.tsx".into()]).unwrap();
    assert_eq!(staged_paths(&dir), ["d.tsx", "i.tsx"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_before_the_first_commit_removes_only_that_entry() {
    let dir = scratch_repo("exact-unstage-unborn");
    for f in ["[id].tsx", "i.tsx", "d.tsx"] {
        write(&dir, f, "x\n");
    }
    git(&dir, &["add", "."]);
    unstage_paths(&dir, &["[id].tsx".into()]).unwrap();
    assert_eq!(staged_paths(&dir), ["d.tsx", "i.tsx"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// HEAD and the index both hold `a.txt` and `A.txt` (different contents), and the index
/// changes both; `core.ignorecase` is on. Nothing is on disk.
fn case_twins(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.ignorecase", "false"]);
    let add = |path: &str, text: &str| {
        let id = blob(&dir, text);
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{id},{path}")]);
    };
    add("a.txt", "lower 1\nlower 2\nlower 3\n");
    add("A.txt", "upper 1\nupper 2\nupper 3\n");
    git(&dir, &["commit", "-q", "-m", "twins"]);
    add("a.txt", "lower 1\nlower two\nlower 3\n");
    add("A.txt", "upper 1\nupper two\nupper 3\n");
    git(&dir, &["config", "core.ignorecase", "true"]);
    dir
}

#[test]
fn unstaging_one_case_twin_leaves_the_other_staged() {
    let dir = case_twins("exact-unstage-twin");
    let upper = staged_id(&dir, "A.txt");
    unstage_paths(&dir, &["a.txt".into()]).unwrap();
    assert_eq!(staged_id(&dir, "a.txt"), git_in(&dir, &["rev-parse", "HEAD:a.txt"], b""));
    assert_eq!(staged_id(&dir, "A.txt"), upper);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_a_hunk_of_one_case_twin_leaves_the_other_alone() {
    for (path, other) in [("a.txt", "A.txt"), ("A.txt", "a.txt")] {
        let dir = case_twins(&format!("exact-hunk-twin-{}", if path == "a.txt" { "lower" } else { "upper" }));
        let kept = staged_id(&dir, other);
        let shown = load_diff(&dir, &DiffTarget::Staged(path.into())).unwrap();
        assert_eq!(shown.hunks.len(), 1);
        apply_hunk(&dir, path, 0, &shown.hunks[0], HunkOp::Unstage).unwrap();
        assert_eq!(staged_id(&dir, path), git_in(&dir, &["rev-parse", &format!("HEAD:{path}")], b""), "{path} unstaged");
        assert_eq!(staged_id(&dir, other), kept, "{other} untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Listing the index's paths against real repositories: order, staged additions, removals,
//! conflicts, submodule links, an empty index and a folder that isn't a repository.

use super::*;
use crate::testutil::*;
use std::path::Path;

fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
}

#[test]
fn tracked_paths_lists_index_in_order() {
    let dir = scratch_repo("tracked-order");
    write(&dir, "a.txt", "a\n");
    write(&dir, "B.txt", "upper case sorts before lower case in byte order\n");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    write(&dir, "src/b.txt", "b\n");
    commit_all(&dir, "first");
    write(&dir, "c.txt", "untracked\n");
    write(&dir, "d.txt", "staged\n");
    git(&dir, &["add", "d.txt"]);
    assert_eq!(tracked_paths(&dir).unwrap(), ["B.txt", "a.txt", "d.txt", "src/b.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracked_paths_after_rm_and_in_conflict() {
    let dir = scratch_repo("tracked-conflict");
    write(&dir, "a.txt", "a\n");
    write(&dir, "shared.txt", "base\n");
    commit_all(&dir, "first");
    git(&dir, &["rm", "-q", "a.txt"]);
    assert_eq!(tracked_paths(&dir).unwrap(), ["shared.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove a"]);

    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "shared.txt", "theirs\n");
    commit_all(&dir, "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "shared.txt", "ours\n");
    commit_all(&dir, "ours");
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "other"]));
    assert!(!ok, "the merge conflicts");
    let index = git2::Repository::open(&dir).unwrap().index().unwrap();
    assert!(index.has_conflicts());
    assert_eq!(tracked_paths(&dir).unwrap(), ["shared.txt"], "the conflicted path appears once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracked_paths_skips_gitlinks() {
    let dir = scratch_repo("tracked-gitlink");
    write(&dir, "a.txt", "a\n");
    write(&dir, "z.txt", "z\n");
    commit_all(&dir, "first");
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap().to_string();
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")]);
    assert_eq!(tracked_paths(&dir).unwrap(), ["a.txt", "z.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracked_paths_empty_and_not_a_repo() {
    let dir = scratch_repo("tracked-empty");
    assert_eq!(tracked_paths(&dir).unwrap(), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);

    let plain = crate::paths::temp_dir().join(format!("glowfly-test-tracked-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&plain);
    std::fs::create_dir_all(&plain).unwrap();
    assert!(tracked_paths(&plain).is_err());
    let _ = std::fs::remove_dir_all(&plain);
}

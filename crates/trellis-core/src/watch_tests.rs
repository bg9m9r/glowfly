//! Classifying watched paths, and the ignored-folder cache.

use super::*;
use crate::testutil::{git, scratch_repo, write};

#[test]
fn git_internals_are_classified() {
    let c = |p: &str| git_change(Path::new(p), Touch::Modified);
    assert_eq!(c("index"), Change::Status);
    assert_eq!(c("info/exclude"), Change::Status);
    assert_eq!(c("info/attributes"), Change::Status);
    assert_eq!(c("info/other"), Change::None);
    for full in ["worktrees/wt/HEAD", "worktrees/wt/locked", "worktrees/wt/gitdir"] {
        assert_eq!(c(full), Change::Full, "{full}");
    }
    for none in ["worktrees/wt/index", "worktrees/wt/logs/HEAD", "worktrees/wt/ORIG_HEAD"] {
        assert_eq!(c(none), Change::None, "{none}");
    }
    for full in [
        "HEAD",
        "packed-refs",
        "refs/heads/main",
        "refs/remotes/origin/x",
        "refs/stash",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REBASE_HEAD",
        "rebase-merge/done",
        "rebase-apply/next",
        "sequencer/todo",
        "config",
        "config.worktree",
    ] {
        assert_eq!(c(full), Change::Full, "{full}");
    }
    for none in [
        "index.lock",
        "refs/heads/main.lock",
        "HEAD.lock",
        "objects/ab/cdef",
        "logs/HEAD",
        "FETCH_HEAD",
        "ORIG_HEAD",
        "COMMIT_EDITMSG",
        "TRELLIS_COMMIT_EDITMSG",
        "config.lock",
        "config.worktree.lock",
        "worktrees/wt/config.worktree",
        "hooks/pre-commit",
        "",
    ] {
        assert_eq!(c(none), Change::None, "{none}");
    }
}

#[test]
fn submodule_repositories_change_the_status() {
    let c = |p: &str| git_change(Path::new(p), Touch::Modified);
    for status in ["modules/libs/lib/HEAD", "modules/x/refs/heads/main", "modules/x/packed-refs", "modules/a/modules/b/HEAD"] {
        assert_eq!(c(status), Change::Status, "{status}");
    }
    for none in [
        "modules/x/logs/HEAD",
        "modules/x/logs/refs/heads/main",
        "modules/x/index",
        "modules/x/objects/ab/cd",
        "modules/x/HEAD.lock",
        "modules/x/FETCH_HEAD",
        "modules/x/config",
        "worktrees/w/modules/x/HEAD",
    ] {
        assert_eq!(c(none), Change::None, "{none}");
    }

    let wd = crate::paths::temp_dir().join("trellis-watch-nested-git-missing").join("wd");
    let paths = WatchPaths::new(&wd, &wd.join(".git"));
    let x = wd.join("vendor").join("x").join(".git");
    let mut never = |_: &str| false;
    assert_eq!(paths.classify_path(&x.join("HEAD"), Touch::Modified, &mut never), (Change::Status, None));
    assert_eq!(paths.classify_path(&x.join("refs").join("heads").join("m"), Touch::Modified, &mut never), (Change::Status, None));
    assert_eq!(paths.classify_path(&x.join("packed-refs"), Touch::Modified, &mut never), (Change::Status, None));
    for none in [x.join("index"), x.join("objects").join("ab").join("cd"), x.join("logs").join("HEAD"), x.join("HEAD.lock"), x.clone()] {
        assert_eq!(paths.classify_path(&none, Touch::Modified, &mut never), (Change::None, None), "{}", none.display());
    }
}

#[test]
fn a_linked_worktree_follows_the_refs_it_shares() {
    let c = |p: &str| common_change(Path::new(p));
    for full in ["refs/heads/main", "refs/remotes/origin/x", "packed-refs", "HEAD", "config"] {
        assert_eq!(c(full), Change::Full, "{full}");
    }
    assert_eq!(c("info/attributes"), Change::Status, "every worktree reads the common attributes");
    assert_eq!(c("info/other"), Change::None);
    // The common dir's `config.worktree` is the main worktree's own.
    for none in [
        "index",
        "refs/heads/main.lock",
        "objects/ab/cd",
        "config.lock",
        "config.worktree",
        "logs/HEAD",
        "rebase-merge/done",
        "worktrees/other/index",
    ] {
        assert_eq!(c(none), Change::None, "{none}");
    }
    // Classified through WatchPaths: the linked worktree's own git dir first, then the common one.
    let common = crate::paths::temp_dir().join("trellis-watch-common-missing").join(".git");
    let git_dir = common.join("worktrees").join("wt");
    let wd = crate::paths::temp_dir().join("trellis-watch-common-missing-wt");
    let paths = WatchPaths::new(&wd, &git_dir).with_common(&common);
    let mut never = |_: &str| false;
    assert_eq!(paths.classify(&common.join("refs/heads/x"), Touch::Other, &mut never), Change::Full);
    assert_eq!(paths.classify(&common.join("index"), Touch::Modified, &mut never), Change::None);
    assert_eq!(paths.classify(&git_dir.join("index"), Touch::Modified, &mut never), Change::Status);
    assert_eq!(paths.classify(&git_dir.join("HEAD"), Touch::Modified, &mut never), Change::Full);
    assert_eq!(paths.classify(&common.join("config"), Touch::Modified, &mut never), Change::Full, "the shared config");
    assert_eq!(paths.classify(&git_dir.join("config.worktree"), Touch::Modified, &mut never), Change::Full, "its own config");
    assert_eq!(paths.classify(&common.join("config.worktree"), Touch::Modified, &mut never), Change::None, "the main worktree's");
    // The main worktree's own git dir is not a second root.
    let main = WatchPaths::new(&wd, &common).with_common(&common);
    assert!(main.common_dirs.is_empty());
}

#[test]
fn common_dir_follows_the_commondir_file() {
    let dir = scratch_repo("watch-commondir");
    let linked = dir.with_file_name(format!("{}-linked", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "linked", &linked.display().to_string()]);
    let repo = git2::Repository::open(&linked).unwrap();
    let common = crate::paths::resolve(&dir.join(".git"));
    assert_eq!(common_dir(repo.path()), Some(common.clone()));
    assert_eq!(common_dir(&common), None, "the main git dir is its own");
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn linked_worktree_folders_count_only_when_they_come_or_go() {
    let c = |p: &str, touch| git_change(Path::new(p), touch);
    // Windows reports these folders modified on every index write in a linked worktree.
    for p in ["worktrees", "worktrees/wt"] {
        assert_eq!(c(p, Touch::Modified), Change::None, "{p}");
        assert_eq!(c(p, Touch::Other), Change::Full, "{p}");
    }
    // The files inside are judged the same way whatever happened to them.
    for touch in [Touch::Modified, Touch::Other] {
        assert_eq!(c("worktrees/wt/gitdir", touch), Change::Full);
        assert_eq!(c("worktrees/wt/index", touch), Change::None);
        assert_eq!(c("worktrees/wt/index.lock", touch), Change::None);
        assert_eq!(c("refs/heads/main", touch), Change::Full);
        assert_eq!(c("index", touch), Change::Status);
    }
}

#[test]
fn the_shallow_file_reloads_everything() {
    for touch in [Touch::Modified, Touch::Other] {
        assert_eq!(git_change(Path::new("shallow"), touch), Change::Full);
        assert_eq!(git_change(Path::new("shallow.lock"), touch), Change::None);
    }
}

#[test]
fn worktree_paths_are_classified() {
    let wd = crate::paths::temp_dir().join("trellis-watch-classify-missing").join("wd");
    let paths = WatchPaths::new(&wd, &wd.join(".git"));
    let mut seen = Vec::new();
    let mut ignored = |rel: &str| {
        seen.push(rel.to_owned());
        rel.starts_with("target/")
    };
    assert_eq!(paths.classify_path(&wd.join("src").join("a.rs"), Touch::Modified, &mut ignored), (Change::Paths, Some("src/a.rs".into())));
    assert_eq!(paths.classify(&wd, Touch::Modified, &mut ignored), Change::None);
    assert_eq!(paths.classify(&wd.join("vendor/sub/.git/index"), Touch::Modified, &mut ignored), Change::None);
    assert_eq!(paths.classify(&wd.join(".git").join("index"), Touch::Modified, &mut ignored), Change::Status, "the git rule wins");
    assert_eq!(paths.classify(&wd.join("target").join("debug").join("x.o"), Touch::Modified, &mut ignored), Change::None);
    let outside = crate::paths::temp_dir().join("trellis-watch-elsewhere").join("a.rs");
    assert_eq!(paths.classify(&outside, Touch::Modified, &mut ignored), Change::None);
    assert_eq!(seen, ["src/a.rs", "target/debug/x.o"], "relative and '/'-separated");

    // A linked worktree: the git dir lives elsewhere and `.git` is a file in the workdir.
    let gd = crate::paths::temp_dir().join("trellis-watch-classify-missing").join("main").join(".git").join("worktrees").join("wd");
    let linked = WatchPaths::new(&wd, &gd);
    assert_eq!(linked.classify(&gd.join("HEAD"), Touch::Modified, &mut |_| false), Change::Full);
    assert_eq!(linked.classify(&wd.join(".git"), Touch::Modified, &mut |_| false), Change::None);

    assert_eq!(Change::Status.max(Change::Full), Change::Full);
    assert_eq!(Change::None.max(Change::Status), Change::Status);
    assert!(Change::None < Change::Paths && Change::Paths < Change::Status && Change::Status < Change::Full);
}

#[test]
fn ignore_and_attribute_files_read_the_whole_status() {
    let wd = crate::paths::temp_dir().join("trellis-watch-whole-missing").join("wd");
    let paths = WatchPaths::new(&wd, &wd.join(".git"));
    let mut never = |_: &str| false;
    for rel in [".gitignore", "sub/.gitignore", ".gitattributes", "a/b/.gitattributes", ".gitmodules"] {
        assert_eq!(paths.classify_path(&wd.join(rel), Touch::Modified, &mut never), (Change::Status, None), "{rel}");
    }
    assert_eq!(paths.classify_path(&wd.join("sub/.gitmodules"), Touch::Modified, &mut never).0, Change::Paths, "only the root's counts");
    for rel in ["index", "info/exclude", "info/attributes"] {
        assert_eq!(paths.classify_path(&wd.join(".git").join(rel), Touch::Modified, &mut never), (Change::Status, None), "{rel}");
    }
    assert_eq!(paths.classify_path(&wd.join(".git/HEAD"), Touch::Modified, &mut never), (Change::Full, None));
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_reads_the_whole_status() {
    use std::os::unix::ffi::OsStrExt;
    let wd = crate::paths::temp_dir().join("trellis-watch-bytes-missing").join("wd");
    let paths = WatchPaths::new(&wd, &wd.join(".git"));
    let name = std::ffi::OsStr::from_bytes(b"caf\xe9.txt");
    assert_eq!(paths.classify_path(&wd.join("dir").join(name), Touch::Modified, &mut |_| false), (Change::Status, None));
}

#[test]
fn ignores_follow_gitignore() {
    let dir = scratch_repo("watch-ignores");
    write(&dir, ".gitignore", "target/\n*.log\n");
    std::fs::create_dir_all(dir.join("target/debug")).unwrap();
    write(&dir, "target/debug/x.o", "obj");
    git(&dir, &["add", ".gitignore"]);
    let mut ig = Ignores::open(&dir);
    assert!(ig.is_ignored("target/debug/x.o"));
    assert!(ig.dirs.contains("target"), "{:?}", ig.dirs);
    assert_eq!(ig.dirs.len(), 1);
    // Answered from the cache: even a repository-less lookup would say no.
    let repo = ig.repo.take();
    assert!(ig.is_ignored("target/release/y.o"));
    ig.repo = repo;
    assert!(ig.is_ignored("a.log"));
    assert!(!ig.is_ignored("src/main.rs"));
    assert!(!ig.is_ignored(".gitignore"));
    ig.clear();
    assert!(ig.dirs.is_empty());

    let plain = crate::paths::temp_dir().join(format!("trellis-watch-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    let mut none = Ignores::open(&plain);
    assert!(!none.is_ignored("target/debug/x.o"));
    assert!(!none.is_ignored("a.log"));
    let _ = std::fs::remove_dir_all(&plain);
    let _ = std::fs::remove_dir_all(&dir);
}

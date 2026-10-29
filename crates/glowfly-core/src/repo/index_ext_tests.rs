//! Reading an index file's extensions, and whether Glowfly's index writes keep git's untracked
//! cache and file system monitor token.

use super::*;
use crate::testutil::{git, scratch_repo, write};

fn exts(dir: &Path) -> Vec<String> {
    index_extensions(&dir.join(".git").join("index")).expect("an index this reader knows")
}

/// A repository with one commit, `a.txt` changed, an untracked file, and git's untracked cache
/// recorded in the index by a status read.
fn cached(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    write(&dir, "b.txt", "two\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "one\nmore\n");
    write(&dir, "new.txt", "untracked\n");
    git(&dir, &["config", "core.untrackedCache", "true"]);
    git(&dir, &["update-index", "--untracked-cache"]);
    git(&dir, &["status", "--porcelain"]);
    dir
}

#[test]
fn extensions_are_read_from_each_index_version() {
    for version in ["2", "3", "4"] {
        let dir = cached(&format!("index-ext-v{version}"));
        git(&dir, &["update-index", "--index-version", version]);
        git(&dir, &["status", "--porcelain"]);
        let found = exts(&dir);
        assert!(found.iter().any(|e| e == UNTRACKED_CACHE), "v{version}: {found:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert_eq!(extensions_in(b"DIRC", 20), None);
    assert_eq!(extensions_in(b"not an index at all, not at all", 20), None);
}

#[test]
fn an_index_without_extensions_has_none() {
    let dir = scratch_repo("index-ext-none");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    assert_eq!(exts(&dir), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

fn has(dir: &Path, sig: &str) -> bool {
    exts(dir).iter().any(|e| e == sig)
}

#[test]
fn glowfly_index_writes_keep_the_untracked_cache() {
    let dir = cached("index-ext-writes");
    let repo = git2::Repository::open(&dir).unwrap();
    assert!(!lost_speedups(&repo));
    // Staging and discarding whole files run git, which keeps the cache.
    crate::repo::stage_file(&dir, "a.txt").unwrap();
    assert!(has(&dir, UNTRACKED_CACHE), "after staging: {:?}", exts(&dir));
    // Unstaging (and hunk staging) write the index through libgit2, which drops it.
    crate::repo::unstage_paths(&dir, &["a.txt".to_owned()]).unwrap();
    assert!(lost_speedups(&repo), "libgit2 keeps only the extensions it knows: {:?}", exts(&dir));
    // Glowfly's next status read lets git record it again; the read after that takes no lock.
    let (unstaged, ..) = crate::repo::status_at(&dir, true).unwrap();
    assert!(unstaged.iter().any(|e| e.path == "new.txt"), "{unstaged:?}");
    assert!(has(&dir, UNTRACKED_CACHE) && !lost_speedups(&repo), "{:?}", exts(&dir));
    crate::repo::discard_file(&dir, "a.txt").unwrap();
    assert!(has(&dir, UNTRACKED_CACHE), "after discarding: {:?}", exts(&dir));
    // Not trusted: libgit2 reads, and nothing is written.
    crate::repo::unstage_paths(&dir, &["b.txt".to_owned()]).unwrap();
    crate::repo::status_at(&dir, false).unwrap();
    assert!(lost_speedups(&repo));
    drop(repo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cache_git_does_not_keep_for_this_read_is_not_asked_for() {
    let dir = cached("index-ext-normal");
    git(&dir, &["config", "status.showUntrackedFiles", "normal"]);
    crate::repo::unstage_paths(&dir, &["a.txt".to_owned()]).unwrap();
    assert!(!lost_speedups(&git2::Repository::open(&dir).unwrap()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The daemon's token, with git's daemon watching `dir` (skipped where git has none).
#[test]
fn glowfly_index_writes_keep_the_monitor_token() {
    let dir = cached("index-ext-monitor");
    let guard = super::super::fsmonitor::tests::Stopped(dir.clone());
    if super::super::fsmonitor::daemon_built(&dir) != Some(true) {
        eprintln!("skipped: this git has no built-in file system monitor");
        return;
    }
    git(&dir, &["fsmonitor--daemon", "start"]);
    git(&dir, &["config", "core.fsmonitor", "true"]);
    git(&dir, &["status", "--porcelain"]);
    git(&dir, &["status", "--porcelain"]);
    assert!(has(&dir, MONITOR_TOKEN), "{:?}", exts(&dir));
    crate::repo::unstage_paths(&dir, &["a.txt".to_owned()]).unwrap();
    let repo = git2::Repository::open(&dir).unwrap();
    assert!(lost_speedups(&repo), "{:?}", exts(&dir));
    crate::repo::status_at(&dir, true).unwrap();
    assert!(has(&dir, MONITOR_TOKEN) && !lost_speedups(&repo), "{:?}", exts(&dir));
    drop(repo);
    drop(guard);
}

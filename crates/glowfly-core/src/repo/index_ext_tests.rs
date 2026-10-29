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

/// Every cut-off prefix of a real index of each version, and the index with any one byte
/// changed, reads as some answer (most often None) without a panic or an endless loop.
#[test]
fn damaged_indexes_never_panic() {
    for version in ["2", "3", "4"] {
        let dir = cached(&format!("index-ext-damaged-v{version}"));
        git(&dir, &["update-index", "--index-version", version]);
        git(&dir, &["status", "--porcelain"]);
        let bytes = std::fs::read(dir.join(".git").join("index")).unwrap();
        assert!(extensions_in(&bytes, 20).is_some(), "v{version} reads whole");
        for hash in [20, 32] {
            for n in 0..bytes.len() {
                let _ = extensions_in(&bytes[..n], hash);
            }
            for at in 0..bytes.len() {
                for flip in [0xff, 0x80, 0x01] {
                    let mut b = bytes.clone();
                    b[at] ^= flip;
                    let _ = extensions_in(&b, hash);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A read that let git record but brought nothing back (here git can't take the index lock) is
/// not repeated on the next refresh; once the index changes, a read records again.
#[test]
fn a_record_that_brings_nothing_back_is_not_repeated() {
    let dir = cached("index-ext-once");
    crate::repo::unstage_paths(&dir, &["a.txt".to_owned()]).unwrap();
    let repo = git2::Repository::open(&dir).unwrap();
    assert!(lost_speedups(&repo));
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    let memo = RecordMemo::default();
    read(&dir, &memo);
    assert!(!has(&dir, UNTRACKED_CACHE) && memo.records() == 1, "{:?}", exts(&dir));
    read(&dir, &memo);
    read(&dir, &memo);
    assert_eq!(memo.records(), 1, "the same index isn't asked again");
    assert!(should_record(&repo, Some(&memo)).is_none());
    std::fs::remove_file(&lock).unwrap();
    crate::repo::unstage_paths(&dir, &["b.txt".to_owned()]).unwrap();
    read(&dir, &memo);
    assert!(has(&dir, UNTRACKED_CACHE) && memo.records() == 2, "{:?}", exts(&dir));
    drop(repo);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A monitor token no daemon here can give (no daemon built, Linux, or Watchman's hook set as
/// a bool by mistake) is never recorded for, and is asked about once per index state.
#[test]
fn a_token_no_daemon_can_give_is_not_recorded_for() {
    let dir = scratch_repo("index-ext-no-daemon");
    write(
        &dir, "a.txt", "one
",
    );
    git(&dir, &["add", "."]);
    git(&dir, &["config", "core.fsmonitor", "true"]);
    let repo = git2::Repository::open(&dir).unwrap();
    let serves = super::super::fsmonitor::daemon_serves(&dir);
    let memo = RecordMemo::default();
    assert_eq!(should_record(&repo, Some(&memo)).is_some(), serves);
    assert!(should_record(&repo, Some(&memo)).is_some() == serves, "remembered when it can't");
    assert!(should_record(&repo, None).is_none(), "no memo, no record");
    drop(repo);
    let _ = std::fs::remove_dir_all(&dir);
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

/// Whether a whole status read of `repo` would let git record its speed-ups, for a memo that
/// has seen no read yet.
fn lost_speedups(repo: &git2::Repository) -> bool {
    should_record(repo, Some(&RecordMemo::default())).is_some()
}

/// A trusted whole status read through `memo`, as the runner makes one.
fn read(dir: &Path, memo: &RecordMemo) -> Vec<crate::repo::FileEntry> {
    crate::repo::status_and_index_with(dir, true, Some(memo)).unwrap().0
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
    // A read lent no memo (a single-shot read) never writes the index.
    crate::repo::status_at(&dir, true).unwrap();
    assert!(lost_speedups(&repo), "{:?}", exts(&dir));
    // Glowfly's next status read lets git record it again; the read after that takes no lock.
    let memo = RecordMemo::default();
    let unstaged = read(&dir, &memo);
    assert!(unstaged.iter().any(|e| e.path == "new.txt"), "{unstaged:?}");
    assert!(has(&dir, UNTRACKED_CACHE) && !lost_speedups(&repo), "{:?}", exts(&dir));
    assert_eq!(memo.records(), 1);
    read(&dir, &memo);
    assert_eq!(memo.records(), 1, "nothing left to record");
    crate::repo::discard_file(&dir, "a.txt").unwrap();
    assert!(has(&dir, UNTRACKED_CACHE), "after discarding: {:?}", exts(&dir));
    // Not trusted: libgit2 reads, and nothing is written.
    crate::repo::unstage_paths(&dir, &["b.txt".to_owned()]).unwrap();
    crate::repo::status_and_index_with(&dir, false, Some(&memo)).unwrap();
    assert!(lost_speedups(&repo) && memo.records() == 1);
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
    read(&dir, &RecordMemo::default());
    assert!(has(&dir, MONITOR_TOKEN) && !lost_speedups(&repo), "{:?}", exts(&dir));
    drop(repo);
    drop(guard);
}

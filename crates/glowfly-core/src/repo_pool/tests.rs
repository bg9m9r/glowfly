//! The repository pool: handles lent again while nothing changed, opened again or brought up to
//! date after git changes the repository, let go before a repack or removal, and expired.

use super::*;
use crate::repo::{DiffTarget, commit_detail, load_diff, refs_key, status_at};
use crate::testutil::{git, scratch_repo, with_submodule, write};
use std::time::SystemTime;

fn opened(path: &Path, how: Use) -> usize {
    pool().opened.iter().find(|(k, h, _)| k == path && *h == how).map_or(0, |e| e.2)
}

fn idle(path: &Path) -> usize {
    pool().idle.iter().filter(|i| i.slot.key == path).count()
}

/// Dates the repository's config and index back, as if written a while ago, so their stamps
/// are trusted (`RACY_WINDOW`).
fn age(dir: &Path) {
    for f in ["config", "index"] {
        if let Ok(file) = std::fs::File::options().write(true).open(dir.join(".git").join(f)) {
            file.set_modified(SystemTime::now() - Duration::from_secs(30)).unwrap();
        }
    }
}

/// A repository with one commit, its files aged.
fn committed(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    age(&dir);
    dir
}

fn staged(dir: &Path) -> Vec<String> {
    status_at(dir, false).unwrap().1.into_iter().map(|e| e.path).collect()
}

fn head(dir: &Path) -> git2::Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn packs(dir: &Path) -> usize {
    let files = std::fs::read_dir(dir.join(".git/objects/pack")).unwrap();
    files.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "pack")).count()
}

#[test]
fn a_handle_is_lent_again_while_nothing_changed() {
    let dir = committed("pool-reuse");
    for _ in 0..3 {
        status_at(&dir, false).unwrap();
        refs_key(&dir).unwrap();
    }
    assert_eq!((opened(&dir, Use::Workdir), opened(&dir, Use::Refs)), (1, 1));
    assert_eq!(idle(&dir), 2, "one handle per use");
    forget(&dir);
}

#[test]
fn a_config_change_made_by_git_opens_it_again() {
    let dir = committed("pool-config");
    status_at(&dir, false).unwrap();
    git(&dir, &["config", "core.autocrlf", "true"]);
    status_at(&dir, false).unwrap();
    assert_eq!(opened(&dir, Use::Workdir), 2);
    forget(&dir);
}

/// Staged and committed by git: the same handle sees both, with or without the index checksum
/// (`index.skipHash` writes zeros instead).
#[test]
fn an_index_written_by_git_is_read_again() {
    for skip in ["false", "true"] {
        let dir = scratch_repo(&format!("pool-index-{skip}"));
        git(&dir, &["config", "index.skipHash", skip]);
        write(&dir, "a.txt", "a\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "a"]);
        age(&dir);
        let key = refs_key(&dir).unwrap();
        assert!(staged(&dir).is_empty());
        // Read straight through `Repository::index`, which libgit2 never reads again itself.
        let entries = || lend(&dir, Use::Workdir).unwrap().index().unwrap().len();
        assert_eq!(entries(), 1);
        write(&dir, "b.txt", "b\n");
        git(&dir, &["add", "b.txt"]);
        assert_eq!(entries(), 2, "skipHash {skip}");
        assert_eq!(staged(&dir), ["b.txt"], "skipHash {skip}");
        git(&dir, &["commit", "-q", "-m", "b"]);
        assert!(staged(&dir).is_empty(), "skipHash {skip}");
        assert_ne!(refs_key(&dir).unwrap(), key);
        assert_eq!(commit_detail(&dir, head(&dir)).unwrap().files.len(), 1);
        assert_eq!(opened(&dir, Use::Workdir), 1, "skipHash {skip}: the index was read again, the repository not reopened");
        forget(&dir);
    }
}

#[test]
fn objects_packed_by_gc_are_still_found() {
    let dir = committed("pool-gc");
    let first = head(&dir);
    commit_detail(&dir, first).unwrap();
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    git(&dir, &["gc", "-q", "--prune=now"]);
    assert_eq!(commit_detail(&dir, head(&dir)).unwrap().message, "b");
    assert_eq!(commit_detail(&dir, first).unwrap().message, "a");
    forget(&dir);
}

/// On Windows an open handle keeps its pack from being deleted, and its folder from being
/// removed: a hold lets go of it, and keeps handles lent meanwhile from staying.
#[test]
fn a_hold_lets_go_of_the_packs_for_a_repack() {
    let dir = committed("pool-hold");
    git(&dir, &["gc", "-q"]);
    age(&dir);
    commit_detail(&dir, head(&dir)).unwrap();
    assert_eq!(idle(&dir), 1);
    {
        let _hold = hold(&dir);
        assert_eq!(idle(&dir), 0);
        commit_detail(&dir, head(&dir)).unwrap();
        assert_eq!(idle(&dir), 0, "not kept while held");
        write(&dir, "b.txt", "b\n");
        git(&dir, &["add", "b.txt"]);
        git(&dir, &["commit", "-q", "-m", "b"]);
        git(&dir, &["repack", "-a", "-d", "-q"]);
        assert_eq!(packs(&dir), 1, "the old pack was deleted");
    }
    commit_detail(&dir, head(&dir)).unwrap();
    assert_eq!(idle(&dir), 1, "kept again once the hold ends");
    forget(&dir);
}

#[test]
fn a_handle_lent_across_a_forget_is_dropped_when_it_comes_back() {
    let dir = committed("pool-lent");
    let lent = lend(&dir, Use::Workdir).unwrap();
    forget(&dir);
    drop(lent);
    assert_eq!(idle(&dir), 0);
}

#[test]
fn a_removed_or_moved_repository_is_not_lent() {
    let dir = committed("pool-removed");
    status_at(&dir, false).unwrap();
    forget(&dir);
    let moved = dir.with_file_name(format!("{}-moved", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&moved);
    std::fs::rename(&dir, &moved).unwrap();
    assert!(status_at(&dir, false).is_err());
    assert!(refs_key(&dir).is_err());
    assert!(status_at(&moved, false).is_ok());
    forget(&moved);
    std::fs::remove_dir_all(&moved).unwrap();
    assert!(status_at(&moved, false).is_err());
}

/// Without a forget the stamps still catch it (where the folder can go while a handle is open).
#[test]
#[cfg(not(windows))]
fn a_repository_removed_under_an_idle_handle_is_not_lent() {
    let dir = committed("pool-removed-idle");
    status_at(&dir, false).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(status_at(&dir, false).is_err());
}

#[test]
fn forget_drops_every_worktree_of_the_repository_but_not_its_submodules() {
    let (parent, sub) = with_submodule("pool-group");
    let wt = parent.with_file_name(format!("{}-wt", parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&parent, &["worktree", "add", "-q", wt.to_str().unwrap()]);
    for d in [&parent, &wt, &sub] {
        status_at(d, false).unwrap();
        assert_eq!(idle(d), 1);
    }
    forget(&wt);
    assert_eq!((idle(&parent), idle(&wt), idle(&sub)), (0, 0, 1));
    forget(&sub);
    git(&parent, &["worktree", "remove", "--force", wt.to_str().unwrap()]);
}

#[test]
fn a_key_keeps_at_most_two_idle_handles_and_they_expire() {
    let dir = committed("pool-cap");
    let lent: Vec<_> = (0..3).map(|_| lend(&dir, Use::Workdir).unwrap()).collect();
    drop(lent);
    assert_eq!(idle(&dir), MAX_PER_KEY);
    // Only this repository's handles: other tests running meanwhile keep theirs.
    let gone = {
        let mut p = pool();
        let (mine, others) = std::mem::take(&mut p.idle).into_iter().partition(|i| i.slot.key == dir);
        p.idle = mine;
        let gone = expire(&mut p, Instant::now() + IDLE);
        p.idle.extend(others);
        gone
    };
    assert!(gone.iter().filter(|i| i.slot.key == dir).count() == MAX_PER_KEY);
    assert_eq!(idle(&dir), 0);
}

#[test]
fn a_diff_sees_the_working_tree_through_a_kept_handle() {
    let dir = committed("pool-diff");
    let unstaged = DiffTarget::Unstaged("a.txt".into());
    write(&dir, "a.txt", "a\nmore\n");
    assert_eq!(load_diff(&dir, &unstaged).unwrap().hunks.len(), 1);
    git(&dir, &["add", "a.txt"]);
    assert!(load_diff(&dir, &unstaged).is_err(), "no unstaged change once git staged it");
    assert_eq!(load_diff(&dir, &DiffTarget::Staged("a.txt".into())).unwrap().hunks.len(), 1);
    assert_eq!(opened(&dir, Use::Workdir), 1);
    forget(&dir);
}

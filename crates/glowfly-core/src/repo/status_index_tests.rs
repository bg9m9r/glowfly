//! An index read after a change made with git, merged into the lists a whole read gave before,
//! must give exactly what a whole read gives now: in libgit2's mode and in git's (untracked cache
//! on).

use super::*;
use crate::repo::{merge_staged, merge_unstaged, status_and_index, status_at};
use crate::testutil::*;
use crate::watch::MAX_WATCH_PATHS;
use std::path::{Path, PathBuf};

type Lists = (Vec<FileEntry>, Vec<FileEntry>);

fn full(dir: &Path, git_status: bool) -> Lists {
    let (unstaged, staged, _) = status_at(dir, git_status).unwrap();
    (unstaged, staged)
}

/// A whole read and the memo it was taken with.
fn before(dir: &Path, git_status: bool) -> (Lists, IndexMemo) {
    let (unstaged, staged, _, memo) = status_and_index(dir, git_status).unwrap();
    ((unstaged, staged), memo.expect("a memo"))
}

/// The staged additions, deletions and renames of `staged`, with the renames' old paths.
fn partners(staged: &[FileEntry]) -> Vec<String> {
    let mut out: Vec<String> = staged
        .iter()
        .filter(|f| matches!(f.status, 'A' | 'D' | 'R'))
        .flat_map(|f| std::iter::once(f.path.clone()).chain(f.renamed_from.clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn read(dir: &Path, was: &(Lists, IndexMemo), watched: &[&str], git_status: bool) -> Option<IndexStatus> {
    let watched: Vec<String> = watched.iter().map(|s| s.to_string()).collect();
    status_after_index_change(dir, &was.1, &partners(&was.0.1), &watched, git_status).unwrap()
}

/// Reads the index change since `was` (it must be readable so), checks the merge against a whole
/// read and returns the read with the merged lists.
fn check(dir: &Path, was: &(Lists, IndexMemo), watched: &[&str], git_status: bool) -> (IndexStatus, Lists) {
    let r = read(dir, was, watched, git_status).expect("read without a whole status");
    let (mut unstaged, mut staged) = was.0.clone();
    merge_staged(&mut staged, &r.paths.covered, r.staged.clone(), false);
    merge_unstaged(&mut unstaged, &r.paths.covered, r.paths.unstaged.clone(), false);
    let merged = (unstaged, staged);
    assert_eq!(merged, full(dir, git_status), "(git: {git_status})");
    (r, merged)
}

fn rows(lists: &[FileEntry]) -> Vec<(&str, char)> {
    lists.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

fn commit_files(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(dir.join(parent)).unwrap();
        }
        write(dir, path, text);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "files"]);
}

/// Runs `case` on a fresh repository holding a.txt and b.txt, with libgit2's read and with git's.
fn both(name: &str, case: impl Fn(&Path, bool)) {
    for git_status in [false, true] {
        let dir = scratch_repo(&format!("{name}-{}", if git_status { "git" } else { "lib" }));
        if git_status {
            set_config(&dir, &[("core.untrackedCache", "true")]);
        }
        commit_files(&dir, &[("a.txt", "one\n"), ("b.txt", "two\n")]);
        case(&dir, git_status);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn staging_an_edit_moves_it_to_staged() {
    both("index-add", |dir, git_status| {
        write(dir, "a.txt", "edited\n");
        let was = before(dir, git_status);
        git(dir, &["add", "a.txt"]);
        let (r, (unstaged, staged)) = check(dir, &was, &[], git_status);
        assert_eq!(r.paths.covered, ["a.txt"]);
        assert_eq!(rows(&staged), [("a.txt", 'M')]);
        assert!(unstaged.is_empty());
        assert_ne!(r.memo, was.1, "the new index");
    });
}

#[test]
fn staging_a_new_file_adds_it() {
    both("index-new", |dir, git_status| {
        write(dir, "new.txt", "new\n");
        let was = before(dir, git_status);
        assert_eq!(rows(&was.0.0), [("new.txt", '?')]);
        git(dir, &["add", "new.txt"]);
        let (_, (unstaged, staged)) = check(dir, &was, &[], git_status);
        assert_eq!(rows(&staged), [("new.txt", 'A')]);
        assert!(unstaged.is_empty());
    });
}

#[test]
fn a_reset_unstages_everything() {
    both("index-reset", |dir, git_status| {
        write(dir, "a.txt", "edited\n");
        write(dir, "new.txt", "new\n");
        git(dir, &["add", "-A"]);
        let was = before(dir, git_status);
        assert_eq!(rows(&was.0.1), [("a.txt", 'M'), ("new.txt", 'A')]);
        git(dir, &["reset", "-q"]);
        let (_, (unstaged, staged)) = check(dir, &was, &[], git_status);
        assert!(staged.is_empty());
        assert_eq!(rows(&unstaged), [("a.txt", 'M'), ("new.txt", '?')]);
    });
}

#[test]
fn a_move_pairs_both_halves() {
    both("index-mv", |dir, git_status| {
        let was = before(dir, git_status);
        git(dir, &["mv", "a.txt", "c.txt"]);
        let (_, (unstaged, staged)) = check(dir, &was, &[], git_status);
        assert_eq!(rows(&staged), [("c.txt", 'R')]);
        assert_eq!(staged[0].renamed_from.as_deref(), Some("a.txt"));
        assert!(unstaged.is_empty());

        // Staging another file keeps the rename paired: its halves are read again too.
        write(dir, "b.txt", "edited\n");
        let was = before(dir, git_status);
        git(dir, &["add", "b.txt"]);
        let (_, (_, staged)) = check(dir, &was, &[], git_status);
        assert_eq!(rows(&staged), [("b.txt", 'M'), ("c.txt", 'R')]);
    });
}

#[test]
fn removing_from_the_index_leaves_an_untracked_file() {
    both("index-rm-cached", |dir, git_status| {
        let was = before(dir, git_status);
        git(dir, &["rm", "-q", "--cached", "a.txt"]);
        let (_, (unstaged, staged)) = check(dir, &was, &[], git_status);
        assert_eq!(rows(&staged), [("a.txt", 'D')]);
        assert_eq!(rows(&unstaged), [("a.txt", '?')]);
    });
}

#[test]
fn a_stat_only_rewrite_reads_nothing() {
    both("index-stat", |dir, git_status| {
        let was = before(dir, git_status);
        write(dir, "a.txt", "one\n");
        git(dir, &["update-index", "--refresh"]);
        let r = read(dir, &was, &[], git_status).expect("no whole read");
        assert!(r.paths.covered.is_empty() && r.paths.unstaged.is_empty() && r.staged.is_empty(), "{r:?}");
        assert_eq!(r.memo.len(), 2);
        assert_eq!(was.0, full(dir, git_status));
    });
}

#[test]
fn the_burst_s_paths_are_read_with_the_change() {
    both("index-watched", |dir, git_status| {
        let was = before(dir, git_status);
        write(dir, "a.txt", "edited\n");
        write(dir, "b.txt", "edited\n");
        git(dir, &["add", "a.txt"]);
        let (r, (unstaged, staged)) = check(dir, &was, &["b.txt"], git_status);
        assert_eq!(r.paths.covered, ["a.txt", "b.txt"]);
        assert_eq!((rows(&staged), rows(&unstaged)), (vec![("a.txt", 'M')], vec![("b.txt", 'M')]));
    });
}

#[test]
fn a_staged_submodule_bump_reads_its_row() {
    for git_status in [false, true] {
        let name = format!("index-sm-{}", if git_status { "git" } else { "lib" });
        let (parent, sub) = with_submodule(&name);
        if git_status {
            set_config(&parent, &[("core.untrackedCache", "true")]);
        }
        write(&sub, "lib.txt", "changed\n");
        git(&sub, &["commit", "-q", "-am", "bump"]);
        let was = before(&parent, git_status);
        assert_eq!(rows(&was.0.0), [("libs/lib", 'M')]);
        git(&parent, &["add", "libs/lib"]);
        let (r, (unstaged, staged)) = check(&parent, &was, &[], git_status);
        assert_eq!(r.paths.covered, ["libs/lib"]);
        assert!(r.paths.submodule_touched);
        assert_eq!(rows(&staged), [("libs/lib", 'M')]);
        assert!(unstaged.is_empty());
        let lib: PathBuf = parent.with_file_name(format!("glowfly-test-{name}-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&parent);
    }
}

#[test]
fn some_changes_read_the_whole_status() {
    let dir = scratch_repo("index-fallbacks");
    commit_files(&dir, &[("a.txt", "one\n")]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "a.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "a.txt", "ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let was = before(&dir, false);
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "-q", "other"]));
    assert!(!ok, "the merge conflicts");
    assert!(read(&dir, &was, &[], false).is_none(), "a conflict");
    git(&dir, &["merge", "--abort"]);

    let was = before(&dir, false);
    write(&dir, ".gitattributes", "*.bin binary\n");
    git(&dir, &["add", ".gitattributes"]);
    assert!(read(&dir, &was, &[], false).is_none(), "an attributes file changes how other paths read");

    let was = before(&dir, false);
    std::fs::create_dir_all(dir.join("many")).unwrap();
    for i in 0..=MAX_WATCH_PATHS {
        write(&dir, &format!("many/f{i}.txt"), "x\n");
    }
    git(&dir, &["add", "many"]);
    assert!(read(&dir, &was, &[], false).is_none(), "more paths than one read takes");
    let _ = std::fs::remove_dir_all(&dir);
}

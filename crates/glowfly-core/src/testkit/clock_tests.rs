//! Pinned dates reach the commits git makes inside a pinned folder, and only there.

use super::{next_for, now, pin, unpin};
use crate::gitcli::{args, output};
use crate::testkit::{git, set_identity};
use git2::Repository;
use std::path::PathBuf;

/// A repository folder in the temporary folder, removed when the test is done with it.
struct Folder(PathBuf);

impl std::ops::Deref for Folder {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(name: &str) -> Folder {
    let dir = crate::paths::temp_dir().join(format!("glowfly-clock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    set_identity(&dir);
    Folder(dir)
}

fn commit(dir: &std::path::Path, msg: &str, env: &[(&str, &str)]) {
    let out = output(dir, &args(&["commit", "--allow-empty", "-m", msg]), "", env, None);
    assert!(out.ok, "commit {msg}: {}", out.stderr);
}

/// (author time, committer time) of HEAD.
fn times(dir: &std::path::Path) -> (i64, i64) {
    let repo = Repository::open(dir).unwrap();
    let c = repo.head().unwrap().peel_to_commit().unwrap();
    (c.author().when().seconds(), c.committer().when().seconds())
}

#[test]
fn pinned_root_gets_increasing_dates() {
    let dir = folder("rising");
    pin(&dir, 1_000_000);
    commit(&dir, "a", &[]);
    assert_eq!(times(&dir), (1_000_000, 1_000_000));
    commit(&dir, "b", &[]);
    assert_eq!(times(&dir), (1_000_060, 1_000_060));
    unpin(&dir);
}

#[test]
fn unpinned_root_uses_the_clock() {
    let dir = folder("unpinned");
    commit(&dir, "a", &[]);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (a, c) = times(&dir);
    assert!((now - a).abs() < 600 && (now - c).abs() < 600, "{a} {c} vs {now}");
}

#[test]
fn caller_dates_win() {
    let dir = folder("caller");
    pin(&dir, 1_000_000);
    commit(&dir, "a", &[("GIT_COMMITTER_DATE", "@2000000000 +0000")]);
    assert_eq!(times(&dir).1, 2_000_000_000);
    unpin(&dir);
}

#[test]
fn two_roots_count_separately() {
    let (a, b) = (folder("root-a"), folder("root-b"));
    pin(&a, 1_000_000);
    pin(&b, 5_000_000);
    commit(&b, "x", &[]);
    commit(&b, "y", &[]);
    commit(&a, "first", &[]);
    assert_eq!(times(&a).1, 1_000_000);
    assert_eq!(times(&b).1, 5_000_060);
    unpin(&a);
    unpin(&b);
}

#[test]
fn now_is_the_pinned_clock_inside_a_pinned_root() {
    let dir = folder("now");
    pin(&dir, 2_000_000);
    assert_eq!(now(&dir.join(".git")), 2_000_000, "the next date, not taken");
    assert_eq!(next_for(&dir), Some(2_000_000));
    assert_eq!(now(&dir), 2_000_060);
    unpin(&dir);
    assert!(now(&dir) > 1_700_000_000, "the real clock once unpinned");
}

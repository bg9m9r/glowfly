//! Tests for `git_ops::local_done` meeting a lock file: what a stage, unstage or discard
//! reports, and that a conflict change only gets the lock sentence.

use glowfly_core::error::WriteFailed;
use glowfly_model::effect::LocalOp;
use glowfly_model::msg::{LockMsg, Msg, StageMsg};
use glowfly_model::testing::{git, scratch_repo};

/// A stale lock beside `dir`'s index.
fn lock_index(dir: &std::path::Path) -> std::path::PathBuf {
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    lock
}

#[test]
fn an_unstage_that_meets_a_lock_reports_it_with_the_op() {
    let dir = scratch_repo("git-ops-lock-unstage");
    git(&dir, &["add", "a.txt"]);
    let lock = lock_index(&dir);
    let op = LocalOp::UnstagePaths(vec!["a.txt".into()]);
    let Msg::Lock(LockMsg::LocalLocked { op: got, lock: named, workdir, error }) = super::local_done(&dir, op.clone()) else {
        panic!("not LocalLocked")
    };
    assert_eq!(*got, op);
    assert_eq!(workdir, dir);
    let resolve = |p: &std::path::Path| glowfly_core::paths::resolve(p);
    assert_eq!(resolve(std::path::Path::new(&named)), resolve(&lock));
    assert!(matches!(error, WriteFailed::Locked { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_change_that_meets_a_lock_offers_no_retry() {
    let dir = scratch_repo("git-ops-lock-conflict");
    git(&dir, &["commit", "-qam", "two"]);
    git(&dir, &["checkout", "-qb", "side"]);
    std::fs::write(dir.join("a.txt"), "side\n").unwrap();
    git(&dir, &["commit", "-qam", "side"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "main\n").unwrap();
    git(&dir, &["commit", "-qam", "main"]);
    let (merged, _) = glowfly_core::testkit::try_git_env(&dir, &["merge", "side"], &[]);
    assert!(!merged, "the merge stops on a conflict");
    lock_index(&dir);
    match super::local_done(&dir, LocalOp::MarkResolved(vec!["a.txt".into()])) {
        Msg::Stage(StageMsg::LocalOpDone(Err(WriteFailed::Locked { .. })) | StageMsg::LocalOpFailed { .. }) => {}
        Msg::Stage(StageMsg::LocalOpDone(Err(other))) => panic!("not worded as a lock: {other:?}"),
        _ => panic!("not a failed conflict change"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

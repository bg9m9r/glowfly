//! Merge, pull, cherry-pick, revert and rebase: recorded as HEAD moving, undone and redone with
//! `reset --keep` while tracked files are unchanged.

use super::tests::{cmds, commit, head_oid, record, refused, run_plan};
use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn on_branch(dir: &Path) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let h = repo.head().unwrap();
    h.is_branch().then(|| h.shorthand().unwrap().to_owned())
}

/// main at c1 (a.txt, gone.txt) and feature with f1 (adds f.txt) and f2 (adds g.txt) on top.
fn forked(name: &str) -> (PathBuf, Oid, Oid, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "gone.txt", "gone\n");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    let f1 = commit(&dir, "f.txt", "f\n", "f1");
    let f2 = commit(&dir, "g.txt", "g\n", "f2");
    git(&dir, &["checkout", "-q", "main"]);
    (dir, c1, f1, f2)
}

/// Undoes `action` (back to `from`) and redoes it (forward to `to`), checking the branch stays.
fn round_trip(dir: &Path, action: &Action, from: Oid, to: Oid) {
    let branch = on_branch(dir);
    let undo = plan(dir, action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--keep", &from.to_string()]]));
    run_plan(dir, undo);
    assert_eq!((head_oid(dir), on_branch(dir)), (from, branch.clone()));
    let redo = plan(dir, action, true).unwrap();
    assert_eq!(redo, cmds(&[&["reset", "--keep", &to.to_string()]]));
    run_plan(dir, redo);
    assert_eq!((head_oid(dir), on_branch(dir)), (to, branch));
}

#[test]
fn merges_undo_and_redo() {
    let (dir, c1, _, f2) = forked("undo-mv-merge");
    let ff = record(&dir, &["merge", "feature"]).expect("recorded");
    let want = Action::Moved { op: MoveOp::Merge, detail: Some("feature".into()), branch: Some("main".into()), from: c1, to: f2 };
    assert_eq!(ff, want);
    assert_eq!(ff.describe(), "merge of feature");
    round_trip(&dir, &ff, c1, f2);
    assert_eq!(record(&dir, &["merge", "feature"]), None, "already up to date");

    run_plan(&dir, plan(&dir, &ff, false).unwrap());
    let m1 = commit(&dir, "a.txt", "main\n", "m1");
    let merge = record(&dir, &["merge", "--no-ff", "--no-edit", "feature"]).expect("recorded");
    let merged = head_oid(&dir);
    assert_eq!(Repository::open(&dir).unwrap().find_commit(merged).unwrap().parent_count(), 2);
    round_trip(&dir, &merge, m1, merged);

    // Local changes to tracked files block it; an unrelated untracked file doesn't.
    write(&dir, "a.txt", "edited\n");
    assert!(matches!(refused(plan(&dir, &merge, false).unwrap()), Refusal::Dirty(m) if m.contains("merge of feature")));
    git(&dir, &["checkout", "--", "a.txt"]);
    write(&dir, "notes.txt", "mine\n");
    assert_eq!(plan(&dir, &merge, false).unwrap(), cmds(&[&["reset", "--keep", &m1.to_string()]]));

    commit(&dir, "b.txt", "later\n", "later");
    let r = refused(plan(&dir, &merge, false).unwrap());
    assert!(r.is_stale() && r.message().starts_with("main has moved since the merge of feature"), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_picks_and_reverts_undo_and_redo() {
    let (dir, c1, f1, f2) = forked("undo-mv-pick");
    let one = record(&dir, &["cherry-pick", &f1.to_string()]).expect("recorded");
    assert_eq!(one.describe(), format!("cherry-pick of {}", &f1.to_string()[..7]));
    let picked = head_oid(&dir);
    round_trip(&dir, &one, c1, picked);
    run_plan(&dir, plan(&dir, &one, false).unwrap());

    let two = record(&dir, &["cherry-pick", &f1.to_string(), &f2.to_string()]).expect("recorded");
    assert_eq!(two.describe(), "cherry-pick of 2 commits");
    let picked = head_oid(&dir);
    round_trip(&dir, &two, c1, picked);

    let revert = record(&dir, &["revert", "--no-edit", &picked.to_string()]).expect("recorded");
    assert!(matches!(&revert, Action::Moved { op: MoveOp::Revert, .. }), "{revert:?}");
    let reverted = head_oid(&dir);
    round_trip(&dir, &revert, picked, reverted);

    // Detached: the undo stays detached.
    git(&dir, &["checkout", "-q", "--detach", &c1.to_string()]);
    let detached = record(&dir, &["cherry-pick", &f1.to_string()]).expect("recorded");
    assert!(matches!(&detached, Action::Moved { branch: None, .. }), "{detached:?}");
    let picked = head_oid(&dir);
    round_trip(&dir, &detached, c1, picked);
    assert_eq!(on_branch(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_undo_and_redo() {
    let (dir, _, _, f2) = forked("undo-mv-rebase");
    commit(&dir, "a.txt", "main\n", "m1");
    git(&dir, &["checkout", "-q", "feature"]);
    let action = record(&dir, &["rebase", "main"]).expect("recorded");
    assert_eq!(action.describe(), "rebase onto main");
    let rebased = head_oid(&dir);
    assert!(matches!(&action, Action::Moved { branch: Some(b), from, .. } if b == "feature" && *from == f2), "{action:?}");
    round_trip(&dir, &action, f2, rebased);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pull_undo_and_redo() {
    let (work, bare) = with_origin("undo-mv-pull");
    let before = head_oid(&work);
    remote_commit(&bare, "main", "b.txt", "remote");
    let action = record(&work, &["pull", "--ff"]).expect("recorded");
    assert_eq!(action.describe(), "pull");
    let pulled = head_oid(&work);
    assert_ne!(pulled, before);
    round_trip(&work, &action, before, pulled);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn untracked_file_in_the_way_fails_and_is_kept() {
    let (dir, _, _, _) = forked("undo-mv-untracked");
    git(&dir, &["checkout", "-q", "feature"]);
    git(&dir, &["rm", "-q", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "drop gone.txt"]);
    git(&dir, &["checkout", "-q", "main"]);
    let action = record(&dir, &["merge", "feature"]).expect("recorded");
    assert!(!dir.join("gone.txt").exists());
    write(&dir, "gone.txt", "mine\n");
    // Refused before git runs (git would refuse too, but not for an ignored file).
    let Plan::Refuse(r) = plan(&dir, &action, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.message().starts_with("Move gone.txt away first"), "{}", r.message());
    assert_eq!(std::fs::read_to_string(dir.join("gone.txt")).unwrap(), "mine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicted_merge_records_nothing() {
    let (dir, _, _, _) = forked("undo-mv-conflict");
    git(&dir, &["checkout", "-q", "feature"]);
    commit(&dir, "a.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "ours\n", "ours");
    let kind = UndoKind::classify(&args(&["merge", "feature"])).unwrap();
    let b = before(&dir, &kind).unwrap();
    assert!(!run(&dir, &args(&["merge", "feature"])).0, "the merge conflicts");
    assert!(!irreversible(&b));
    assert_eq!(after(&dir, kind, b).unwrap(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

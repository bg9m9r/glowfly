//! A restack recorded as one undoable action: every branch it moved goes back, and forward
//! again on redo.

use super::tests::{cmds, commit, refused, run_plan};
use super::*;
use crate::stack::{self, RestackScope};
use crate::testutil::*;
use std::path::{Path, PathBuf};

const DETAIL: &str = "restack of the branches above low";

fn tip(dir: &Path, name: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(name).unwrap().id()
}

/// main <- low <- mid <- top, low amended, then `head` checked out.
fn amended(name: &str, head: &str) -> PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, "base.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l.txt", "l\n", "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m.txt", "m\n", "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    commit(&dir, "t.txt", "t\n", "t1");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    git(&dir, &["checkout", "-q", head]);
    dir
}

/// Restacks the branches above low, recording it; returns the action and the old mid and top.
fn restacked_action(dir: &Path) -> (Action, Oid, Oid) {
    let (mid, top) = (tip(dir, "mid"), tip(dir, "top"));
    let snap = crate::repo::load(dir, &crate::repo::LoadOptions::all(100)).unwrap();
    let plan = stack::plan(&snap.stacks, &RestackScope::Above("low".into()), snap.head_branch.as_deref(), false);
    let before = restack_before(dir).unwrap();
    assert!(restack_unmoved(dir, &before));
    assert_eq!(restacked(dir, &before, DETAIL), None, "nothing moved yet");
    let run = stack::restack(dir, &plan).unwrap();
    assert!(matches!(run.outcome, crate::rebase::Outcome::Finished { .. }), "{:?}", run.outcome);
    assert!(!restack_unmoved(dir, &before));
    (restacked(dir, &before, DETAIL).unwrap(), mid, top)
}

#[test]
fn a_restack_is_undone_and_redone_branch_by_branch() {
    let dir = amended("undo-restacked", "low");
    let (action, mid, top) = restacked_action(&dir);
    let (new_mid, new_top) = (tip(&dir, "mid"), tip(&dir, "top"));
    let want = vec![BranchMove { name: "mid".into(), from: mid, to: new_mid }, BranchMove { name: "top".into(), from: top, to: new_top }];
    assert_eq!(action, Action::Restacked { detail: DETAIL.into(), moves: want });
    assert_eq!(action.describe(), DETAIL);

    let undo = plan(&dir, &action, false).unwrap();
    let reason = "Glowfly: undo restack";
    let (m, t, nm, nt) = (mid.to_string(), top.to_string(), new_mid.to_string(), new_top.to_string());
    assert_eq!(
        undo,
        cmds(&[&["update-ref", "-m", reason, "refs/heads/mid", &m, &nm], &["update-ref", "-m", reason, "refs/heads/top", &t, &nt]])
    );
    run_plan(&dir, undo);
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (mid, top));
    run_plan(&dir, plan(&dir, &action, true).unwrap());
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (new_mid, new_top));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_checked_out_branch_is_reset_last() {
    let dir = amended("undo-restacked-head", "mid");
    let (action, mid, top) = restacked_action(&dir);
    assert_eq!(git2::Repository::open(&dir).unwrap().head().unwrap().shorthand(), Some("mid"));
    let new_top = tip(&dir, "top");
    let undo = plan(&dir, &action, false).unwrap();
    let (t, nt) = (top.to_string(), new_top.to_string());
    assert_eq!(
        undo,
        cmds(&[&["update-ref", "-m", "Glowfly: undo restack", "refs/heads/top", &t, &nt], &["reset", "--keep", &mid.to_string()]])
    );

    write(&dir, "m.txt", "dirty\n");
    assert!(matches!(refused(plan(&dir, &action, false).unwrap()), Refusal::Dirty(_)));
    git(&dir, &["checkout", "--", "m.txt"]);
    run_plan(&dir, plan(&dir, &action, false).unwrap());
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (mid, top));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_moved_since_refuses_as_stale() {
    let dir = amended("undo-restacked-stale", "low");
    let (action, _, _) = restacked_action(&dir);
    git(&dir, &["branch", "-f", "top", "main"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(r.is_stale() && r.message().contains("Branch top has moved since the restack"), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

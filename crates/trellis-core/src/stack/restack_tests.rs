//! Restacking real repositories with git: replaying the upper layers onto a rewritten lower
//! branch, stopping on a conflict and going on, refusing a stale plan.

use super::read_tests::{commit, stacks_of, tip};
use super::*;
use crate::rebase::{Outcome, Resume};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// main <- low (l1) <- mid (m1) <- top (t1), with low checked out and amended, so mid and top
/// need a restack.
fn amended(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "g.txt", "1\n2\n3\n4\n5\n6\n7\n8\n");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    write(&dir, "g.txt", "one\n2\n3\n4\n5\n6\n7\n8\n");
    commit(&dir, "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    commit(&dir, "t1");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    dir
}

fn above_low(dir: &Path, autostash: bool) -> RestackPlan {
    let head = crate::repo::load(dir, &crate::repo::LoadOptions::all(100)).unwrap().head_branch;
    plan(&stacks_of(dir), &RestackScope::Above("low".into()), head.as_deref(), autostash)
}

fn head_name(dir: &Path) -> String {
    git2::Repository::open(dir).unwrap().head().unwrap().shorthand().unwrap().to_owned()
}

fn parent(dir: &Path, rev: &str) -> Oid {
    tip(dir, &format!("{rev}^"))
}

fn count(dir: &Path, range: &str) -> usize {
    let (ok, out) = crate::gitcli::run(dir, &crate::gitcli::args(&["rev-list", "--count", range]));
    assert!(ok, "{out}");
    out.trim().parse().unwrap()
}

#[test]
fn the_layers_above_an_amended_branch_move_onto_it() {
    let dir = amended("stack-restack-amend");
    let p = above_low(&dir, false);
    assert_eq!(p.steps.len(), 1, "{p:?}");
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { stash_kept: false, .. }), "{:?}", run.outcome);
    assert!(run.left.is_empty() && run.stayed_on.is_none());
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"));
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"));
    assert_eq!(count(&dir, "low..mid"), 1, "mid's own commit is replayed once");
    assert_eq!(head_name(&dir), "low", "the branch checked out before is checked out again");
    assert!(stacks_of(&dir).links.iter().all(|l| !l.behind));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fork_moves_both_leaves_onto_the_new_middle() {
    let dir = amended("stack-restack-fork");
    git(&dir, &["checkout", "-q", "-b", "top2", "mid"]);
    commit(&dir, "t2");
    git(&dir, &["checkout", "-q", "low"]);
    let p = above_low(&dir, false);
    assert_eq!(p.steps.len(), 2, "{p:?}");
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"));
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"));
    assert_eq!(parent(&dir, "top2"), tip(&dir, "mid"));
    assert_eq!(head_name(&dir), "low");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_stops_and_continue_runs_the_rest() {
    let dir = amended("stack-restack-conflict");
    git(&dir, &["checkout", "-q", "-b", "top2", "mid"]);
    commit(&dir, "t2");
    git(&dir, &["checkout", "-q", "low"]);
    // low now changes the line mid changed.
    write(&dir, "g.txt", "uno\n2\n3\n4\n5\n6\n7\n8\n");
    git(&dir, &["commit", "-q", "-a", "--amend", "-m", "l1, amended again"]);
    let p = above_low(&dir, false);
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Stopped { .. }), "{:?}", run.outcome);
    assert_eq!(run.left.steps.len(), 1, "the second leaf is left");
    assert_eq!(run.left.return_to.as_deref(), Some("low"));
    write(&dir, "g.txt", "both\n2\n3\n4\n5\n6\n7\n8\n");
    git(&dir, &["add", "g.txt"]);
    let run = resume(&dir, Resume::Continue, &run.left).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"));
    assert_eq!(parent(&dir, "top2"), tip(&dir, "mid"));
    assert_eq!(head_name(&dir), "low");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_plan_changes_nothing() {
    let dir = amended("stack-restack-stale");
    let p = above_low(&dir, false);
    git(&dir, &["branch", "-f", "top", "main"]);
    let (mid, top) = (tip(&dir, "mid"), tip(&dir, "top"));
    let err = restack(&dir, &p).unwrap_err();
    assert_eq!(err.to_string(), STALE_PLAN);
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (mid, top));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autostash_keeps_uncommitted_changes() {
    let dir = amended("stack-restack-autostash");
    write(&dir, "base.txt", "changed\n");
    let p = above_low(&dir, true);
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { stash_kept: false, .. }), "{:?}", run.outcome);
    assert_eq!(std::fs::read_to_string(dir.join("base.txt")).unwrap(), "changed\n");
    assert_eq!(head_name(&dir), "low");
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refusing_pre_rebase_hook_moves_nothing() {
    let dir = amended("stack-restack-hook");
    let hook = dir.join(".git/hooks/pre-rebase");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (mid, top) = (tip(&dir, "mid"), tip(&dir, "top"));
    let p = above_low(&dir, false);
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Failed { .. }), "{:?}", run.outcome);
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (mid, top));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_checkout_back_stays_on_the_top() {
    let dir = amended("stack-restack-stay");
    // A change to a line mid didn't touch: it applies on top, but checking low out again would
    // overwrite it.
    write(&dir, "g.txt", "1\n2\n3\n4\n5\n6\n7\neight\n");
    let p = above_low(&dir, true);
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(run.stayed_on.as_deref(), Some("top"));
    assert_eq!(head_name(&dir), "top");
    assert_eq!(std::fs::read_to_string(dir.join("g.txt")).unwrap(), "one\n2\n3\n4\n5\n6\n7\neight\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn head_on_a_middle_layer_moves_with_it_and_comes_back() {
    let dir = amended("stack-restack-middle");
    git(&dir, &["checkout", "-q", "mid"]);
    let p = above_low(&dir, false);
    assert_eq!(p.return_to.as_deref(), Some("mid"));
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"), "a branch checked out here is moved too");
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"));
    assert_eq!(head_name(&dir), "mid");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_layer_behind_on_a_behind_layer_moves_onto_its_new_tip() {
    // main <- low <- mid (m1, m2) <- top (built on m1), then low amended: mid is behind on low
    // and top behind on mid.
    let dir = scratch_repo("stack-restack-two-behind");
    write(&dir, "g.txt", "1\n2\n3\n4\n5\n6\n7\n8\n");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    commit(&dir, "t1");
    git(&dir, &["checkout", "-q", "mid"]);
    commit(&dir, "m2");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    let p = above_low(&dir, false);
    assert_eq!(p.steps.len(), 2, "{p:?}");
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(tip(&dir, "mid~2"), tip(&dir, "low"), "mid keeps both its commits");
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"), "top sits on mid's new tip");
    assert_eq!(count(&dir, "low..top"), 3, "m1 is replayed once, m2 kept, t1 on top");
    assert!(stacks_of(&dir).links.iter().all(|l| !l.behind));
    let _ = std::fs::remove_dir_all(&dir);
}

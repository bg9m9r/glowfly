//! Restacking a three-layer stack (main <- low <- mid <- top) with uncommitted changes: they
//! are stashed once before the first step and come back, staged as they were, on the branch
//! checked out before: after the last step, after a stop and Continue, and after an abort.

use super::read_tests::{commit, tip};
use super::restack_tests::{above_low, amended, head_name, parent};
use super::*;
use crate::rebase::{Outcome, Resume};
use crate::stash_ops::{self, PutBack};
use crate::testkit::git_out;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// A staged change to base.txt and an unstaged one to l1.txt, neither touched by the stack.
fn dirty(dir: &Path) {
    write(dir, "base.txt", "staged\n");
    git(dir, &["add", "base.txt"]);
    write(dir, "l1.txt", "unstaged\n");
}

/// The changes `dirty` made are back, with base.txt staged and l1.txt not.
#[track_caller]
fn changes_back(dir: &Path) {
    assert_eq!(std::fs::read_to_string(dir.join("base.txt")).unwrap(), "staged\n");
    assert_eq!(std::fs::read_to_string(dir.join("l1.txt")).unwrap(), "unstaged\n");
    assert_eq!(git_out(dir, &["diff", "--cached", "--name-only"]).trim(), "base.txt", "what was staged is staged");
    assert_eq!(git_out(dir, &["diff", "--name-only"]).trim(), "l1.txt");
}

fn stashes(dir: &Path) -> usize {
    stash_ops::list(dir).unwrap().len()
}

/// main <- low (l1) <- mid (m1, m2) <- top (t1, on m1), low checked out and amended: two
/// steps, the second of which conflicts (m2 and t1 both change g.txt's first line).
fn second_step_conflicts(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "g.txt", "1\n2\n3\n");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    write(&dir, "g.txt", "three\n2\n3\n");
    commit(&dir, "t1");
    git(&dir, &["checkout", "-q", "mid"]);
    write(&dir, "g.txt", "two\n2\n3\n");
    commit(&dir, "m2");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    dir
}

#[test]
fn changes_are_stashed_once_and_come_back_on_the_original_branch() {
    let dir = amended("stack-stash-once");
    dirty(&dir);
    let p = above_low(&dir, true);
    let run = restack(&dir, &p).unwrap();
    let Outcome::Finished { output, stash_kept: false } = &run.outcome else { panic!("{:?}", run.outcome) };
    assert_eq!(output.matches("$ git stash push").count(), 1, "{output}");
    assert!(!output.contains("--autostash"), "{output}");
    assert!(matches!(run.put_back, Some((_, PutBack::Restored))), "{:?}", run.put_back);
    assert!(run.stayed_on.is_none() && run.left.stash.is_none());
    assert_eq!(head_name(&dir), "low");
    assert_eq!(parent(&dir, "mid"), tip(&dir, "low"));
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"));
    changes_back(&dir);
    assert_eq!(stashes(&dir), 0, "the stash is gone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_tree_stashes_nothing() {
    let dir = amended("stack-stash-clean");
    let run = restack(&dir, &above_low(&dir, true)).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(run.put_back, None);
    assert_eq!(stashes(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stop_keeps_the_stash_until_continue_ends_the_restack() {
    let dir = second_step_conflicts("stack-stash-stop");
    dirty(&dir);
    let p = above_low(&dir, true);
    assert_eq!(p.steps.len(), 2, "{p:?}");
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Stopped { .. }), "{:?}", run.outcome);
    let oid = run.left.stash.expect("the stash waits with the steps left");
    assert_eq!(run.put_back, None);
    assert_eq!(stash_ops::list(&dir).unwrap()[0].oid, oid);
    assert!(stash_ops::list(&dir).unwrap()[0].message.contains("Auto-stash before restacking onto low"));
    assert_eq!(std::fs::read_to_string(dir.join("l1.txt")).unwrap(), "l1\n", "the changes are out of the way");

    write(&dir, "g.txt", "both\n2\n3\n");
    git(&dir, &["add", "g.txt"]);
    let run = resume(&dir, Resume::Continue, &run.left).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(run.put_back, Some((oid, PutBack::Restored)));
    assert_eq!(head_name(&dir), "low");
    assert_eq!(parent(&dir, "top"), tip(&dir, "mid"));
    changes_back(&dir);
    assert_eq!(stashes(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_abort_goes_back_to_the_original_branch_with_the_changes() {
    let dir = second_step_conflicts("stack-stash-abort");
    let old_top = tip(&dir, "top");
    dirty(&dir);
    let run = restack(&dir, &above_low(&dir, true)).unwrap();
    assert!(matches!(run.outcome, Outcome::Stopped { .. }), "{:?}", run.outcome);
    let oid = run.left.stash.unwrap();
    git(&dir, &["rebase", "--abort"]);
    assert_eq!(head_name(&dir), "top", "git's abort leaves HEAD on the step's top");

    let end = end_stopped(&dir, &run.left, Some("top")).unwrap();
    assert!(matches!(end.outcome, Outcome::Finished { .. }), "{:?}", end.outcome);
    assert_eq!(end.put_back, Some((oid, PutBack::Restored)));
    assert_eq!(head_name(&dir), "low");
    changes_back(&dir);
    assert_eq!(stashes(&dir), 0);
    assert_eq!(tip(&dir, "mid~2"), tip(&dir, "low"), "the finished step stays done");
    assert_eq!(tip(&dir, "top"), old_top, "the aborted step moved nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ending_on_another_branch_than_the_stop_keeps_the_stash() {
    let dir = second_step_conflicts("stack-stash-elsewhere");
    dirty(&dir);
    let run = restack(&dir, &above_low(&dir, true)).unwrap();
    let oid = run.left.stash.unwrap();
    git(&dir, &["rebase", "--abort"]);
    git(&dir, &["checkout", "-q", "main"]);
    let end = end_stopped(&dir, &run.left, Some("top")).unwrap();
    assert!(matches!(end.put_back, Some((o, PutBack::Failed)) if o == oid), "{:?}", end.put_back);
    assert_eq!(head_name(&dir), "main", "a branch checked out since is left alone");
    assert_eq!(stash_ops::list(&dir).unwrap()[0].oid, oid, "the stash stays listed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_checkout_back_keeps_the_stash() {
    // HEAD on side (side.txt tracked there only); a post-rewrite hook leaves an untracked
    // side.txt, so checking side out again after the last step is refused.
    let dir = amended("stack-stash-refused");
    git(&dir, &["checkout", "-q", "-b", "side", "main"]);
    commit(&dir, "side");
    write(&dir, "base.txt", "changed\n");
    let hook = dir.join(".git/hooks/post-rewrite");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\necho in-the-way > side.txt\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let p = above_low(&dir, true);
    assert_eq!(p.return_to.as_deref(), Some("side"));
    let run = restack(&dir, &p).unwrap();
    assert!(matches!(run.outcome, Outcome::Finished { .. }), "{:?}", run.outcome);
    assert_eq!(run.stayed_on.as_deref(), Some("top"));
    let Some((oid, PutBack::Failed)) = run.put_back else { panic!("{:?}", run.put_back) };
    assert_eq!(stash_ops::list(&dir).unwrap()[0].oid, oid, "not popped onto the wrong branch");
    assert_eq!(std::fs::read_to_string(dir.join("base.txt")).unwrap(), "base\n");
    let _ = std::fs::remove_dir_all(&dir);
}

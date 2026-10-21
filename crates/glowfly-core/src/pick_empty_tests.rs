//! Cherry-picks and reverts that change nothing, against real git in fixture repositories.

use super::*;
use crate::gitcli::{args, run};
use crate::pick_ops::{cherry_pick_cmd, revert_cmd};
use crate::testutil::{git, scratch_repo, write};
use git2::RepositoryState;
use std::path::PathBuf;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn state(dir: &Path) -> RepositoryState {
    Repository::open(dir).unwrap().state()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

/// Runs `cmd` the way the app's pick job does, then carries it past the empty commits.
fn pick(dir: &Path, picking: Picking, oids: &[Oid], cmd: &[String], can_skip: bool) -> (bool, String, PickResult) {
    let start = Some(head(dir));
    let first = run(dir, cmd);
    skip_empty(dir, picking, oids, start, first, can_skip, &mut |c| run(dir, c))
}

/// main: base; `side`: three commits adding x1.txt, x2.txt and x3.txt. Back on main.
fn three_on_side(name: &str) -> (PathBuf, [Oid; 3]) {
    let dir = scratch_repo(name);
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let x = [1, 2, 3].map(|n| commit(&dir, &format!("x{n}.txt"), "x\n", &format!("x{n}")));
    git(&dir, &["checkout", "-q", "main"]);
    (dir, x)
}

#[test]
fn a_pick_the_branch_already_has_is_skipped() {
    let (dir, x) = three_on_side("empty-pick-one");
    git(&dir, &["cherry-pick", &x[0].to_string()]);
    let before = head(&dir);
    let (ok, out) = run(&dir, &cherry_pick_cmd(&[x[0]], None));
    assert!(!ok && stopped_empty(&dir, &out), "{out}");
    git(&dir, &["cherry-pick", "--abort"]);
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &[x[0]], &cherry_pick_cmd(&[x[0]], None), true);
    assert!(ok, "{out}");
    assert_eq!((r.skipped, r.total, r.left_empty), (vec![x[0]], 1, None));
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sequence_skips_only_the_commits_already_applied() {
    let (dir, x) = three_on_side("empty-pick-middle");
    git(&dir, &["cherry-pick", &x[1].to_string()]);
    let before = head(&dir);
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &x, &cherry_pick_cmd(&x, None), true);
    assert!(ok, "{out}");
    assert_eq!(r.skipped, [x[1]]);
    assert_eq!(r.made(), 2);
    assert_eq!(state(&dir), RepositoryState::Clean);
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.graph_ahead_behind(head(&dir), before).unwrap(), (2, 0), "two new commits");
    assert!(dir.join("x1.txt").exists() && dir.join("x3.txt").exists());

    // Everything already applied: all skipped, nothing made.
    let now = head(&dir);
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &x, &cherry_pick_cmd(&x, None), true);
    assert!(ok, "{out}");
    assert_eq!(r.skipped, x);
    assert_eq!(r.made(), 0);
    assert_eq!((head(&dir), state(&dir)), (now, RepositoryState::Clean));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_already_undone_is_reported() {
    let dir = scratch_repo("empty-revert-one");
    commit(&dir, "a.txt", "a\n", "base");
    let d = commit(&dir, "d.txt", "d\n", "add d");
    git(&dir, &["revert", "--no-edit", &d.to_string()]);
    let before = head(&dir);
    let (ok, out) = run(&dir, &revert_cmd(&[d], None));
    assert!(!ok && stopped_empty(&dir, &out), "{out}");
    assert!(Repository::open(&dir).unwrap().find_reference("REVERT_HEAD").is_err(), "git leaves no state");
    let (ok, out, r) = pick(&dir, Picking::Revert, &[d], &revert_cmd(&[d], None), true);
    assert!(ok, "{out}");
    assert_eq!(r.skipped, [d]);
    assert_eq!((head(&dir), state(&dir)), (before, RepositoryState::Clean));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverting_a_merge_then_its_side_commit_skips_the_side() {
    let dir = scratch_repo("empty-revert-merge");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let side = commit(&dir, "s.txt", "s\n", "side work");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let merge = head(&dir);
    let oids = [merge, side];
    let (ok, out) = run(&dir, &revert_cmd(&oids, Some(1)));
    assert!(!ok && stopped_empty(&dir, &out), "{out}");
    assert_eq!(stopped_at(&dir, Picking::Revert, &oids, Some(merge), 0), Some(side), "one revert made, so the second commit");
    let (ok, out, r) = skip_empty(&dir, Picking::Revert, &oids, Some(merge), (ok, out), true, &mut |c| run(&dir, c));
    assert!(ok, "{out}");
    assert_eq!(r.skipped, [side]);
    assert_eq!(r.made(), 1);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert!(!dir.join("s.txt").exists(), "the merge's revert took the side's file away");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_real_conflict_is_left_stopped() {
    let dir = scratch_repo("empty-pick-conflict");
    commit(&dir, "f.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let clash = commit(&dir, "f.txt", "side\n", "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", "main\n", "main edit");
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &[clash], &cherry_pick_cmd(&[clash], None), true);
    assert!(!ok, "{out}");
    assert!(!stopped_empty(&dir, &out));
    assert!(r.skipped.is_empty() && r.left_empty.is_none());
    assert!(matches!(state(&dir), RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{:?}", state(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_skip_a_single_pick_is_aborted_and_a_sequence_stops() {
    let (dir, x) = three_on_side("empty-pick-old-git");
    git(&dir, &["cherry-pick", &x[0].to_string()]);
    let before = head(&dir);
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &[x[0]], &cherry_pick_cmd(&[x[0]], None), false);
    assert!(ok, "{out}");
    assert_eq!((r.skipped, r.left_empty), (vec![x[0]], None));
    assert_eq!((head(&dir), state(&dir)), (before, RepositoryState::Clean));

    let two = [x[0], x[1]];
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &two, &cherry_pick_cmd(&two, None), false);
    assert!(!ok, "{out}");
    assert_eq!((r.skipped, r.left_empty), (vec![], Some(x[0])));
    assert_eq!(state(&dir), RepositoryState::CherryPickSequence);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stopped_at_counts_the_commits_made_since_the_start() {
    let dir = scratch_repo("empty-stopped-at");
    commit(&dir, "a.txt", "a\n", "base");
    let d = commit(&dir, "d.txt", "d\n", "add d");
    git(&dir, &["revert", "--no-edit", &d.to_string()]);
    let e = commit(&dir, "e.txt", "e\n", "add e");
    let start = head(&dir);
    let (ok, out) = run(&dir, &revert_cmd(&[e, d], None));
    assert!(!ok, "the second revert is empty: {out}");
    assert!(Repository::open(&dir).unwrap().find_reference("REVERT_HEAD").is_err());
    assert_eq!(stopped_at(&dir, Picking::Revert, &[e, d], Some(start), 0), Some(d));
    assert_eq!(stopped_at(&dir, Picking::Revert, &[e, d], Some(start), 1), None, "out of range");
    git(&dir, &["revert", "--skip"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_what_git_said_counts() {
    let dir = scratch_repo("empty-signs");
    commit(&dir, "a.txt", "a\n", "base");
    assert!(stopped_empty(&dir, "On branch main\nnothing to commit, working tree clean"));
    assert!(stopped_empty(&dir, "no changes added to commit (use \"git add\")"));
    assert!(stopped_empty(&dir, "The previous cherry-pick is now empty, possibly due to conflict resolution."));
    assert!(!stopped_empty(&dir, "hint: nothing to commit\n$ git commit -m nothing to commit"));
    assert!(!stopped_empty(&dir, "error: your local changes would be overwritten by cherry-pick."));
    assert!(!stopped_empty(&dir, "[main 1a2b3c4] Stop polling when the queue is now empty"));
    assert!(!stopped_empty(&dir, "[main 1a2b3c4] Revert \"nothing to commit\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_subject_that_sounds_empty_does_not_skip_a_real_failure() {
    let dir = scratch_repo("empty-pick-subject");
    commit(
        &dir, "a.txt", "a
", "base",
    );
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let a = commit(
        &dir,
        "q.txt",
        "q
",
        "Stop polling when the queue is now empty, nothing to commit",
    );
    let b = commit(
        &dir,
        "config.local",
        "side
",
        "add config.local",
    );
    git(&dir, &["checkout", "-q", "main"]);
    write(
        &dir,
        "config.local",
        "mine
",
    );
    let before = head(&dir);
    let (ok, out, r) = pick(&dir, Picking::CherryPick, &[a, b], &cherry_pick_cmd(&[a, b], None), true);
    assert!(!ok, "the untracked file stops the second pick: {out}");
    assert!(out.contains("is now empty"), "git echoes the first commit's subject: {out}");
    assert!(!stopped_empty(&dir, &out), "{out}");
    assert!(r.skipped.is_empty() && r.left_empty.is_none(), "{r:?}");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.graph_ahead_behind(head(&dir), before).unwrap(), (1, 0), "only the first commit made");
    assert!(repo.path().join("sequencer").is_dir(), "left stopped at the second commit");
    assert_eq!(
        std::fs::read_to_string(dir.join("config.local")).unwrap(),
        "mine
"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_and_abort_command_lines() {
    assert_eq!(args(Picking::CherryPick.skip()), ["cherry-pick", "--skip"]);
    assert_eq!(args(Picking::Revert.skip()), ["revert", "--skip"]);
    assert_eq!(args(Picking::CherryPick.abort()), ["cherry-pick", "--abort"]);
    assert_eq!(args(Picking::Revert.abort()), ["revert", "--abort"]);
}

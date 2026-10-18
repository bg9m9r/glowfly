//! Merges, cherry-picks and reverts that stopped for conflicts: finishing them (Continue, Skip or
//! a commit) records one move from the HEAD they started at.

use super::tests::{cmds, commit, head_oid, record, run_plan};
use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// Runs `git <a>` as an undoable operation whether or not it succeeds: (ok, recorded).
fn attempt(dir: &Path, a: &[&str]) -> (bool, Option<Action>) {
    let kind = UndoKind::classify(&args(a)).expect("an undoable command");
    let b = before(dir, &kind).unwrap();
    let (ok, _) = run(dir, &args(a));
    (ok, after(dir, kind, b).unwrap())
}

fn conflicts(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(!ok, "git {a:?} should stop for conflicts: {out}");
}

/// Resolves `file` to `text` and stages it.
fn resolve(dir: &Path, file: &str, text: &str) {
    write(dir, file, text);
    git(dir, &["add", file]);
}

fn short_of(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// base (a.txt "one", b.txt "b"); `other` with q1 (a.txt), q2 (b.txt), r (adds c.txt) and q3
/// (a.txt again); main with an edit to a.txt and b.txt that q1, q2 and q3 conflict with.
/// Returns (dir, main's tip, [q1, q2, r, q3]).
fn picks(name: &str) -> (PathBuf, Oid, [Oid; 4]) {
    let dir = scratch_repo(name);
    write(&dir, "b.txt", "b\n");
    commit(&dir, "a.txt", "one\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    let q1 = commit(&dir, "a.txt", "one\nq1\n", "q1");
    let q2 = commit(&dir, "b.txt", "b\nq2\n", "q2");
    let r = commit(&dir, "c.txt", "c\n", "r");
    let q3 = commit(&dir, "a.txt", "one\nq1\nq3\n", "q3");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "b.txt", "b\nmain\n");
    let tip = commit(&dir, "a.txt", "one\nmain\n", "main edit");
    (dir, tip, [q1, q2, r, q3])
}

/// Undoes `action` back to `from` and redoes it forward to `to`.
fn round_trip(dir: &Path, action: &Action, from: Oid, to: Oid) {
    let undo = plan(dir, action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--keep", &from.to_string()]]));
    run_plan(dir, undo);
    assert_eq!(head_oid(dir), from);
    let redo = plan(dir, action, true).unwrap();
    assert_eq!(redo, cmds(&[&["reset", "--keep", &to.to_string()]]));
    run_plan(dir, redo);
    assert_eq!(head_oid(dir), to);
}

/// main and other both edit a.txt's second line. Returns (dir, main's tip).
fn merge_conflict(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    commit(&dir, "a.txt", "one\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "a.txt", "one\ntheirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    let tip = commit(&dir, "a.txt", "one\nours\n", "ours");
    (dir, tip)
}

#[test]
fn merge_concluded_by_a_commit_is_a_move() {
    let (dir, tip) = merge_conflict("undo-rs-merge");
    conflicts(&dir, &["merge", "other"]);
    resolve(&dir, "a.txt", "one\nmerged\n");
    let action = record(&dir, &["commit", "-q", "-m", "merged"]).expect("recorded");
    let merged = head_oid(&dir);
    let want = Action::Moved { op: MoveOp::Merge, detail: Some("other".into()), branch: Some("main".into()), from: tip, to: merged };
    assert_eq!(action, want);
    assert_eq!(action.describe(), "merge of other");
    round_trip(&dir, &action, tip, merged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_continued_is_a_move() {
    let (dir, tip) = merge_conflict("undo-rs-merge-continue");
    conflicts(&dir, &["merge", "other"]);
    resolve(&dir, "a.txt", "one\nmerged\n");
    let action = record(&dir, &["merge", "--continue"]).expect("recorded");
    assert!(matches!(&action, Action::Moved { op: MoveOp::Merge, from, .. } if *from == tip), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remote_branch_named() {
    let (dir, tip) = merge_conflict("undo-rs-merge-remote");
    git(&dir, &["update-ref", "refs/remotes/origin/x", "other"]);
    git(&dir, &["branch", "-q", "-D", "other"]);
    conflicts(&dir, &["merge", "origin/x"]);
    resolve(&dir, "a.txt", "one\nmerged\n");
    let action = record(&dir, &["commit", "-q", "-m", "merged"]).expect("recorded");
    let Action::Moved { op: MoveOp::Merge, detail, from, .. } = &action else { panic!("{action:?}") };
    assert_eq!((detail.as_deref(), *from), (Some("refs/remotes/origin/x"), tip));
    assert_eq!(action.describe(), "merge of origin/x");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn single_pick_continued() {
    let (dir, tip, [q1, ..]) = picks("undo-rs-pick");
    conflicts(&dir, &["cherry-pick", &q1.to_string()]);
    resolve(&dir, "a.txt", "one\nmain\nq1\n");
    let action = record(&dir, &["cherry-pick", "--continue"]).expect("recorded");
    let to = head_oid(&dir);
    let want = Action::Moved { op: MoveOp::CherryPick, detail: Some(short_of(q1)), branch: Some("main".into()), from: tip, to };
    assert_eq!(action, want);
    round_trip(&dir, &action, tip, to);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn single_pick_committed_from_the_commit_box() {
    let (dir, tip, [q1, ..]) = picks("undo-rs-pick-commit");
    conflicts(&dir, &["cherry-pick", &q1.to_string()]);
    resolve(&dir, "a.txt", "one\nmain\nq1\n");
    let action = record(&dir, &["commit", "-q", "-m", "q1"]).expect("recorded");
    assert!(matches!(&action, Action::Moved { op: MoveOp::CherryPick, from, .. } if *from == tip), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_continued() {
    let dir = scratch_repo("undo-rs-revert");
    commit(&dir, "a.txt", "one\n", "base");
    let a = commit(&dir, "a.txt", "one\nA\n", "A");
    let b = commit(&dir, "a.txt", "one\nB\n", "B");
    conflicts(&dir, &["revert", "--no-edit", &a.to_string()]);
    resolve(&dir, "a.txt", "one\n");
    let action = record(&dir, &["revert", "--continue"]).expect("recorded");
    let to = head_oid(&dir);
    let want = Action::Moved { op: MoveOp::Revert, detail: Some(short_of(a)), branch: Some("main".into()), from: b, to };
    assert_eq!(action, want);
    assert_eq!(action.describe(), format!("revert of {}", short_of(a)));
    round_trip(&dir, &action, b, to);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sequence_continued_from_the_original_head() {
    let (dir, tip, [q1, q2, r, _]) = picks("undo-rs-seq");
    // r first, so one pick lands before the stop at q1.
    conflicts(&dir, &["cherry-pick", &r.to_string(), &q1.to_string(), &q2.to_string()]);
    assert_ne!(head_oid(&dir), tip, "r was applied before the stop");
    resolve(&dir, "a.txt", "one\nmain\nq1\n");
    // q2 conflicts in turn; resolve it and finish.
    let (ok, action) = attempt(&dir, &["cherry-pick", "--continue"]);
    assert!(!ok && action.is_none(), "stopped again at q2: {action:?}");
    resolve(&dir, "b.txt", "b\nmain\nq2\n");
    let action = record(&dir, &["cherry-pick", "--continue"]).expect("recorded");
    let Action::Moved { op: MoveOp::CherryPick, detail, from, to, .. } = &action else { panic!("{action:?}") };
    assert_eq!((*from, *to), (tip, head_oid(&dir)), "from the HEAD before the whole sequence");
    assert_eq!(detail.as_deref(), Some("3 commits"));
    assert_eq!(action.describe(), "cherry-pick of 3 commits");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_that_stops_again_records_nothing() {
    let (dir, tip, [q1, q2, ..]) = picks("undo-rs-stops-again");
    conflicts(&dir, &["cherry-pick", &q1.to_string(), &q2.to_string()]);
    resolve(&dir, "a.txt", "one\nmain\nq1\n");
    let (ok, action) = attempt(&dir, &["cherry-pick", "--continue"]);
    assert!(!ok, "q2 conflicts too");
    assert_eq!(action, None);
    resolve(&dir, "b.txt", "b\nmain\nq2\n");
    let action = record(&dir, &["cherry-pick", "--continue"]).expect("recorded");
    assert!(matches!(&action, Action::Moved { from, detail: Some(d), .. } if *from == tip && d == "2 commits"), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_that_ends_a_sequence() {
    let (dir, tip, [q1, _, r, q3]) = picks("undo-rs-skip");
    conflicts(&dir, &["cherry-pick", &q1.to_string(), &r.to_string()]);
    let action = record(&dir, &["cherry-pick", "--skip"]).expect("recorded");
    assert!(matches!(&action, Action::Moved { from, detail: Some(d), .. } if *from == tip && d == "1 commit"), "{action:?}");
    assert_eq!(action.describe(), "cherry-pick of 1 commit");

    // Every pick skipped: HEAD is back where it started, so nothing to undo.
    run_plan(&dir, plan(&dir, &action, false).unwrap());
    conflicts(&dir, &["cherry-pick", &q1.to_string(), &q3.to_string()]);
    let (ok, action) = attempt(&dir, &["cherry-pick", "--skip"]);
    assert!(!ok && action.is_none(), "stopped at q3: {action:?}");
    let (ok, action) = attempt(&dir, &["cherry-pick", "--skip"]);
    assert!(ok, "the sequence ended");
    assert_eq!(action, None);
    assert_eq!(head_oid(&dir), tip);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_box_commit_mid_sequence_records_nothing() {
    let (dir, tip, [q1, _, r, _]) = picks("undo-rs-mid-commit");
    conflicts(&dir, &["cherry-pick", &q1.to_string(), &r.to_string()]);
    resolve(&dir, "a.txt", "one\nmain\nq1\n");
    let kind = UndoKind::classify(&args(&["commit", "-q", "-m", "q1"])).unwrap();
    let b = before(&dir, &kind).unwrap();
    git(&dir, &["commit", "-q", "-m", "q1"]);
    assert_eq!(after(&dir, kind, b).unwrap(), None, "the sequence is still pending");
    let action = record(&dir, &["cherry-pick", "--continue"]).expect("recorded");
    assert!(matches!(&action, Action::Moved { from, detail: Some(d), .. } if *from == tip && d == "2 commits"), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn later_commit_after_a_mid_sequence_commit_is_still_a_commit() {
    let (dir, _, [q1, _, r, _]) = picks("undo-rs-after-mid-commit");
    conflicts(&dir, &["cherry-pick", &q1.to_string(), &r.to_string()]);
    resolve(
        &dir,
        "a.txt",
        "one
main
q1
",
    );
    git(&dir, &["commit", "-q", "-m", "q1"]);
    assert!(dir.join(".git/sequencer/todo").is_file(), "git keeps the rest of the sequence");
    // An ordinary commit in that state is recorded as a commit, not swallowed by the sequence.
    let from = head_oid(&dir);
    write(
        &dir, "d.txt", "fix
",
    );
    git(&dir, &["add", "d.txt"]);
    let action = record(&dir, &["commit", "-q", "-m", "fix"]).expect("recorded");
    assert!(matches!(&action, Action::Commit { from: f, amend: false, message, .. } if *f == from && message == "fix"), "{action:?}");
    let action = record(&dir, &["commit", "-q", "--allow-empty", "-m", "again"]).expect("recorded");
    assert!(matches!(&action, Action::Commit { message, .. } if message == "again"), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_without_operation_in_progress_is_still_a_commit() {
    let dir = scratch_repo("undo-rs-plain-commit");
    let base = commit(&dir, "a.txt", "one\n", "base");
    write(&dir, "a.txt", "two\n");
    git(&dir, &["add", "."]);
    let action = record(&dir, &["commit", "-q", "-m", "second"]).expect("recorded");
    assert!(matches!(&action, Action::Commit { from, amend: false, .. } if *from == base), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

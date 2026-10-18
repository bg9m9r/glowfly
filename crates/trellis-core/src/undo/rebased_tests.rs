//! Rebases recorded as one undoable action: where the branch was before (HEAD, or git's
//! orig-head while one is in progress) and where it went.

use super::tests::{cmds, commit, head_oid, run_plan};
use super::*;
use crate::rebase::{self, Outcome, RebaseAction, RebaseBase, SequenceEditor, StartRequest, TodoStep};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// main: c1..c3.
fn three(name: &str) -> (PathBuf, [Oid; 3]) {
    let dir = scratch_repo(name);
    let c1 = commit(&dir, "1.txt", "1\n", "c1");
    let c2 = commit(&dir, "2.txt", "2\n", "c2");
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    (dir, [c1, c2, c3])
}

fn run_rebase(dir: &Path, from: Oid, edit: impl FnOnce(&mut Vec<TodoStep>)) -> Outcome {
    let p = rebase::plan(dir, &RebaseBase::FromCommit(from)).unwrap();
    let mut items: Vec<TodoStep> = p.steps;
    edit(&mut items);
    let req = StartRequest { head: p.head, upstream: p.upstream, items, autostash: false };
    rebase::start(dir, &req, &SequenceEditor::Copy).unwrap()
}

#[test]
fn start_on_a_clean_branch_and_during_a_rebase() {
    let (dir, [_, c2, c3]) = three("undo-rebased-start");
    let s = rebase_start(&dir).unwrap();
    assert_eq!((s.branch.as_str(), s.from, s.interactive), ("main", c3, false));
    assert_eq!(s.describe(), "rebase of main");

    let outcome = run_rebase(&dir, c2, |items| items[0].commit_mut().unwrap().action = RebaseAction::Edit);
    assert!(matches!(outcome, Outcome::Stopped { .. }));
    let s = rebase_start(&dir).unwrap();
    assert_eq!((s.branch.as_str(), s.from, s.interactive), ("main", c3, true));
    assert_eq!(s.describe(), "interactive rebase of main");
    assert!(!unmoved(&dir, &s), "HEAD is detached while the rebase runs");
    git(&dir, &["rebase", "--abort"]);
    assert!(unmoved(&dir, &s));

    git(&dir, &["checkout", "-q", "--detach", "HEAD"]);
    assert!(rebase_start(&dir).is_none(), "a detached HEAD has no branch to undo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_finished_rebase_is_undone_and_redone() {
    let (dir, [_, c2, c3]) = three("undo-rebased-undo");
    let start = rebase_start(&dir).unwrap();
    assert_eq!(rebased(&dir, &start, "reword of x"), None, "nothing moved yet");
    let outcome = run_rebase(&dir, c2, |items| {
        let item = items[0].commit_mut().unwrap();
        item.action = RebaseAction::Reword;
        item.new_message = Some("c2, reworded".into());
    });
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    let to = head_oid(&dir);
    let short = c2.to_string()[..7].to_owned();
    let detail = format!("reword of {short}");
    let action = rebased(&dir, &start, &detail).unwrap();
    let want = Action::Moved { op: MoveOp::Rewrite, detail: Some(detail.clone()), branch: Some("main".into()), from: c3, to };
    assert_eq!(action, want);
    assert_eq!(action.describe(), detail);

    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--keep", &c3.to_string()]]));
    run_plan(&dir, undo);
    assert_eq!(head_oid(&dir), c3, "the branch is back where it was");
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["reset", "--keep", &to.to_string()]]));
    run_plan(&dir, redo);
    assert_eq!(head_oid(&dir), to);
    let _ = std::fs::remove_dir_all(&dir);
}

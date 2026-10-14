//! Running interactive rebases in fixture repositories, with `cp` as the sequence editor.

use super::plan_tests::{commit, head, linear};
use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn request(dir: &Path, base: RebaseBase, edit: impl FnOnce(&mut Vec<TodoItem>)) -> StartRequest {
    let p = plan(dir, &base).unwrap();
    let mut items = p.items;
    edit(&mut items);
    StartRequest { head: p.head, upstream: p.upstream, items }
}

fn start_copy(dir: &Path, req: &StartRequest) -> Outcome {
    start(dir, req, &SequenceEditor::Copy).unwrap()
}

fn out(dir: &Path, a: &[&str]) -> String {
    let (ok, text) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {text}");
    text
}

fn log(dir: &Path) -> Vec<String> {
    out(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

fn message(dir: &Path, rev: &str) -> String {
    out(dir, &["log", "-1", "--format=%B", rev]).trim_end().to_owned()
}

fn state(dir: &Path) -> git2::RepositoryState {
    Repository::open(dir).unwrap().state()
}

fn set(items: &mut [TodoItem], i: usize, action: RebaseAction) {
    items[i].action = action;
}

#[test]
fn reorder_and_drop() {
    let (dir, [_, c2, ..]) = linear("rebase-run-reorder");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        // c2 c3 c4 -> c4, (c2 dropped), c3
        items.rotate_right(1);
        set(items, 1, RebaseAction::Drop);
    });
    let outcome = start_copy(&dir, &req);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["c3", "c4", "c1"]);
    assert!(!dir.join("2.txt").exists(), "the dropped commit's change is gone");
    assert!(dir.join("3.txt").exists() && dir.join("4.txt").exists());
    assert_eq!(state(&dir), git2::RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multi_line_reword() {
    let (dir, [_, c2, _, c4]) = linear("rebase-run-reword");
    let new = "New title\n\n# kept: whitespace cleanup leaves this line\nAnd a body.";
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        set(items, 0, RebaseAction::Reword);
        items[0].new_message = Some(new.into());
    });
    let outcome = start_copy(&dir, &req);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(message(&dir, "HEAD~2"), new);
    assert_eq!(log(&dir), ["c4", "c3", "New title", "c1"], "later commits are still on top");
    let repo = Repository::open(&dir).unwrap();
    let old_tree = repo.find_commit(c4).unwrap().tree_id();
    assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().tree_id(), old_tree, "the tree is unchanged");
    let reworded = repo.revparse_single("HEAD~2").unwrap().peel_to_commit().unwrap();
    assert_eq!(reworded.author().name(), Some("Test"));
    assert_eq!(reworded.tree_id(), repo.find_commit(c2).unwrap().tree_id());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn squash_and_fixup() {
    let (dir, [_, c2, ..]) = linear("rebase-run-squash");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| set(items, 1, RebaseAction::Squash));
    assert!(matches!(start_copy(&dir, &req), Outcome::Finished { .. }));
    assert_eq!(log(&dir).len(), 3);
    let m = message(&dir, "HEAD~1");
    assert!(m.contains("c2") && m.contains("The second commit.") && m.contains("c3"), "{m}");
    assert!(!m.lines().any(|l| l.starts_with('#')), "comment lines are stripped: {m}");
    let files = out(&dir, &["show", "--name-only", "--format=", "HEAD~1"]);
    assert_eq!(files.lines().collect::<Vec<_>>(), ["2.txt", "3.txt"], "the changes are combined");

    // Fixup keeps only the first message.
    let first = head(&dir);
    let base = Repository::open(&dir).unwrap().find_commit(first).unwrap().parent_id(0).unwrap();
    let req = request(&dir, RebaseBase::FromCommit(base), |items| set(items, 1, RebaseAction::Fixup));
    assert!(matches!(start_copy(&dir, &req), Outcome::Finished { .. }));
    assert_eq!(message(&dir, "HEAD"), m);
    assert!(dir.join("4.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_stops_and_continue_finishes() {
    let (dir, [_, c2, ..]) = linear("rebase-run-edit");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        set(items, 0, RebaseAction::Edit);
        set(items, 2, RebaseAction::Reword);
        items[2].new_message = Some("c4, reworded".into());
    });
    let outcome = start_copy(&dir, &req);
    match &outcome {
        Outcome::Stopped { ok: true, stop, .. } => assert_eq!(*stop, Stop { kind: StopKind::Edit, oid: Some(c2) }),
        other => panic!("expected an edit stop, got {other:?}"),
    }
    assert_eq!(state(&dir), git2::RepositoryState::RebaseInteractive);
    assert_eq!(stopped_at(&dir).map(|s| s.kind), Some(StopKind::Edit));
    out(&dir, &["rebase", "--continue"]);
    assert_eq!(state(&dir), git2::RepositoryState::Clean);
    assert_eq!(log(&dir), ["c4, reworded", "c3", "c2", "c1"], "the reword after the stop is applied");
    assert_eq!(stopped_at(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reordering_conflicting_commits_stops_on_the_conflict() {
    let dir = scratch_repo("rebase-run-conflict");
    let base = commit(&dir, "f.txt", "base\n", "base");
    let a = commit(&dir, "f.txt", "one\n", "a");
    let b = commit(&dir, "f.txt", "two\n", "b");
    let req = request(&dir, RebaseBase::FromCommit(a), |items| items.reverse());
    let outcome = start_copy(&dir, &req);
    match &outcome {
        Outcome::Stopped { ok: false, stop, .. } => assert_eq!(*stop, Stop { kind: StopKind::Conflict, oid: Some(b) }),
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(state(&dir), git2::RepositoryState::RebaseInteractive);
    out(&dir, &["rebase", "--abort"]);
    assert_eq!(head(&dir), b, "abort restores the original HEAD");
    assert_ne!(head(&dir), base);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_requests_change_nothing() {
    let (dir, [_, c2, ..]) = linear("rebase-run-stale");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| items.reverse());
    let moved = commit(&dir, "5.txt", "5\n", "c5");
    let rebase_dir = Repository::open(&dir).unwrap().path().join("rebase-merge");
    let e = start(&dir, &req, &SequenceEditor::Copy).unwrap_err();
    assert_eq!(e.to_string(), STALE);
    assert_eq!(head(&dir), moved);
    assert!(!rebase_dir.exists());

    // The commit set differs from what git would rebase.
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        items.pop();
    });
    assert_eq!(start(&dir, &req, &SequenceEditor::Copy).unwrap_err().to_string(), CHANGED);
    assert_eq!(head(&dir), moved);

    // A failing sequence editor.
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| items.reverse());
    match start(&dir, &req, &SequenceEditor::Shell("false".into())) {
        Err(_) | Ok(Outcome::Failed { .. }) => {}
        Ok(other) => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(state(&dir), git2::RepositoryState::Clean);
    assert_eq!(head(&dir), moved);
    assert!(!rebase_dir.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rewording_the_root_commit() {
    let (dir, [c1, ..]) = linear("rebase-run-root");
    let req = request(&dir, RebaseBase::FromCommit(c1), |items| {
        set(items, 0, RebaseAction::Reword);
        items[0].new_message = Some("The very first commit".into());
    });
    assert_eq!(req.upstream, Upstream::Root);
    assert!(matches!(start_copy(&dir, &req), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c3", "c2", "The very first commit"]);
    let _ = std::fs::remove_dir_all(&dir);
}

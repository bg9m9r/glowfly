//! Running interactive rebases in fixture repositories, with `cp` as the sequence editor.

use super::plan_tests::{commit, head, linear};
use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

pub(super) fn request(dir: &Path, base: RebaseBase, edit: impl FnOnce(&mut Vec<TodoItem>)) -> StartRequest {
    let p = plan(dir, &base).unwrap();
    let mut items = p.items;
    edit(&mut items);
    StartRequest { head: p.head, upstream: p.upstream, items, autostash: false }
}

pub(super) fn start_copy(dir: &Path, req: &StartRequest) -> Outcome {
    start(dir, req, &SequenceEditor::Copy).unwrap()
}

pub(super) fn out(dir: &Path, a: &[&str]) -> String {
    let (ok, text) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {text}");
    text
}

pub(super) fn log(dir: &Path) -> Vec<String> {
    out(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

pub(super) fn message(dir: &Path, rev: &str) -> String {
    out(dir, &["log", "-1", "--format=%B", rev]).trim_end().to_owned()
}

pub(super) fn state(dir: &Path) -> git2::RepositoryState {
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
        Outcome::Stopped { ok: true, progress, .. } => assert_eq!(progress.stop, Stop { kind: StopKind::Edit, oid: Some(c2) }),
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
        Outcome::Stopped { ok: false, progress, .. } => assert_eq!(progress.stop, Stop { kind: StopKind::Conflict, oid: Some(b) }),
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

#[test]
fn skipping_a_reworded_pick_rewords_nothing_else() {
    let dir = scratch_repo("rebase-run-reword-skip");
    let base = commit(&dir, "f.txt", "base\n", "base");
    let a = commit(&dir, "f.txt", "one\n", "a");
    let b = commit(&dir, "f.txt", "two\n", "b");
    let req = request(&dir, RebaseBase::FromCommit(a), |items| {
        // b first conflicts on top of base; it is reworded.
        items.reverse();
        set(items, 0, RebaseAction::Reword);
        items[0].new_message = Some("New b message".into());
    });
    match start_copy(&dir, &req) {
        Outcome::Stopped { progress, .. } => assert_eq!(progress.stop, Stop { kind: StopKind::Conflict, oid: Some(b) }),
        other => panic!("expected a conflict, got {other:?}"),
    }
    out(&dir, &["rebase", "--skip"]);
    assert_eq!(state(&dir), git2::RepositoryState::Clean);
    assert_eq!(log(&dir), ["a", "base"], "the skipped commit's message goes nowhere");
    assert_eq!(head(&dir), a, "a replays onto base unchanged");
    assert_eq!(Repository::open(&dir).unwrap().find_commit(a).unwrap().parent_id(0).unwrap(), base);
    let _ = std::fs::remove_dir_all(&dir);
}

fn stash_tip(dir: &Path) -> Option<Oid> {
    Repository::open(dir).unwrap().refname_to_id("refs/stash").ok()
}

#[test]
fn autostash_keeps_changes() {
    let (dir, [_, _, c3, _]) = linear("rebase-run-autostash");
    write(
        &dir,
        "1.txt",
        "1, edited
",
    );
    let before = head(&dir);
    let reword = |dir: &Path, autostash: bool| {
        let mut req = request(dir, RebaseBase::FromCommit(c3), |items| {
            set(items, 0, RebaseAction::Reword);
            items[0].new_message = Some("c3, reworded".into());
        });
        req.autostash = autostash;
        req
    };
    // Without autostash git refuses and nothing changes.
    let outcome = start_copy(&dir, &reword(&dir, false));
    assert!(matches!(outcome, Outcome::Failed { .. }), "{outcome:?}");
    assert_eq!((state(&dir), head(&dir)), (git2::RepositoryState::Clean, before));

    let outcome = start_copy(&dir, &reword(&dir, true));
    assert!(matches!(outcome, Outcome::Finished { stash_kept: false, .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["c4", "c3, reworded", "c2", "c1"]);
    assert_eq!(
        std::fs::read_to_string(dir.join("1.txt")).unwrap(),
        "1, edited
",
        "the edit is back"
    );
    assert_eq!(stash_tip(&dir), None, "nothing is left in the stash list");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autostash_conflict_is_reported() {
    let (dir, [_, c2, ..]) = linear("rebase-run-autostash-conflict");
    write(
        &dir,
        "2.txt",
        "2, edited
",
    );
    let mut req = request(&dir, RebaseBase::FromCommit(c2), |items| set(items, 0, RebaseAction::Drop));
    req.autostash = true;
    let outcome = start_copy(&dir, &req);
    assert!(matches!(outcome, Outcome::Finished { stash_kept: true, .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["c4", "c3", "c1"]);
    assert!(stash_tip(&dir).is_some(), "git kept the changes in the stash list");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refused_todo_with_autostash_restores_changes() {
    let (dir, [_, c2, ..]) = linear("rebase-run-autostash-refused");
    write(
        &dir,
        "1.txt",
        "1, edited
",
    );
    let before = head(&dir);
    let mut req = request(&dir, RebaseBase::FromCommit(c2), |items| items.reverse());
    req.autostash = true;
    match start(&dir, &req, &SequenceEditor::Shell("false".into())) {
        Err(_) | Ok(Outcome::Failed { .. }) => {}
        Ok(other) => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!((state(&dir), head(&dir)), (git2::RepositoryState::Clean, before));
    assert_eq!(
        std::fs::read_to_string(dir.join("1.txt")).unwrap(),
        "1, edited
",
        "the edit is back"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

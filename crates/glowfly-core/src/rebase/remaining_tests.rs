//! Reading the remaining steps of a running rebase (also one started outside Glowfly) and
//! replacing them with `git rebase --edit-todo`.

use super::plan_tests::{commit, linear};
use super::run_tests::{at, log, message, request, request_steps, start_copy, state};
use super::*;
use crate::error::Error;
use crate::gitcli::{args, run, run_with_input};
use crate::testutil::*;
use git2::RepositoryState;
use std::path::{Path, PathBuf};

/// Starts `git rebase -i <upstream>` the way a terminal would, with a sequence editor that
/// writes `todo` as the list. Returns git's exit status.
pub(super) fn rebase_i(dir: &Path, upstream: &str, todo: &str) -> bool {
    let file = dir.join(".git").join("test-todo");
    std::fs::write(&file, todo).unwrap();
    let editor = editor_command(&SequenceEditor::Copy, &file);
    let (ok, _) = run_with_input(dir, &args(&["rebase", "-i", upstream]), "", &[("GIT_SEQUENCE_EDITOR", &editor)]);
    ok
}

fn refused(r: crate::error::Result<Remaining>) -> String {
    match r {
        Err(Error::Unsupported(t)) => t,
        Err(e) => panic!("expected a refusal, got {e:?}"),
        Ok(r) => panic!("expected a refusal, got {r:?}"),
    }
}

fn kinds(steps: &[TodoStep]) -> Vec<String> {
    steps
        .iter()
        .map(|s| match s {
            TodoStep::Commit(i) => format!("{} {}", i.action.keyword(), i.summary),
            TodoStep::Exec(c) => format!("exec {c}"),
            TodoStep::Break => "break".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn edit_request(r: &Remaining, steps: Vec<TodoStep>) -> EditTodoRequest {
    EditTodoRequest { steps, locked_text: r.locked_text.clone(), todo: r.todo.clone(), first_may_combine: r.first_may_combine }
}

fn todo_text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".git").join("rebase-merge").join("git-rebase-todo")).unwrap()
}

#[test]
fn refused_without_an_interactive_rebase() {
    let dir = scratch_repo("rebase-remaining-none");
    commit(&dir, "f.txt", "base\n", "base");
    assert_eq!(refused(remaining(&dir)), NOT_INTERACTIVE);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "f.txt", "other\n", "other");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", "mine\n", "mine");
    let (ok, _) = run(&dir, &args(&["rebase", "--merge", "other"]));
    assert!(!ok, "the commit conflicts");
    // Recent git marks every merge-backend rebase interactive; older git's plain merge
    // rebase leaves the marker out, which is what this checks.
    let _ = std::fs::remove_file(dir.join(".git").join("rebase-merge").join("interactive"));
    assert_eq!(state(&dir), RepositoryState::RebaseMerge);
    assert_eq!(refused(remaining(&dir)), NOT_INTERACTIVE);
    let req = EditTodoRequest { steps: Vec::new(), locked_text: String::new(), todo: String::new(), first_may_combine: true };
    let e = edit_todo(&dir, &req, &SequenceEditor::Copy).unwrap_err();
    assert_eq!(e.to_string(), NOT_INTERACTIVE);
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lists_a_rebase_started_elsewhere() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-remaining-outside");
    let todo = format!("edit {c2} c2\nexec echo one\npick {c3} c3\nbreak\nreword {c4} c4\n");
    assert!(rebase_i(&dir, &c1.to_string(), &todo));
    let r = remaining(&dir).unwrap();
    assert_eq!(kinds(&r.steps), ["exec echo one", "pick c3", "break", "reword c4"]);
    assert_eq!(r.branch.as_deref(), Some("main"));
    assert_eq!(r.onto_label, format!("{} c1", &c1.to_string()[..7]));
    assert_eq!((r.done, r.first_may_combine), (1, true));
    assert!(r.locked.is_empty() && r.locked_text.is_empty());
    let c4_item = r.steps[3].commit().unwrap();
    assert_eq!((c4_item.oid, c4_item.new_message.as_deref(), c4_item.author.as_str()), (c4, None, "Test"));
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn glowfly_rewords_fold_back() {
    let (dir, [_, c2, ..]) = linear("rebase-remaining-fold");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        items[0].action = RebaseAction::Edit;
        items[2].action = RebaseAction::Reword;
        items[2].new_message = Some("c4, reworded\n\nWith a body.".into());
    });
    assert!(matches!(start_copy(&dir, &req), Outcome::Stopped { .. }));
    let r = remaining(&dir).unwrap();
    assert_eq!(kinds(&r.steps), ["pick c3", "reword c4"]);
    assert_eq!(r.steps[1].commit().unwrap().new_message.as_deref(), Some("c4, reworded\n\nWith a body."));
    assert!(r.locked.is_empty());

    // A squash group's combined message folds back into its head.
    git(&dir, &["rebase", "--abort"]);
    let req = request_steps(&dir, RebaseBase::FromCommit(c2), |steps| {
        at(steps, 0).action = RebaseAction::Edit;
        at(steps, 2).action = RebaseAction::Squash;
        at(steps, 1).combined_message = Some("Three and four".into());
    });
    assert!(matches!(start_copy(&dir, &req), Outcome::Stopped { .. }));
    let r = remaining(&dir).unwrap();
    assert_eq!(kinds(&r.steps), ["pick c3", "squash c4"]);
    assert_eq!(r.steps[0].commit().unwrap().combined_message.as_deref(), Some("Three and four"));

    // A reworded head of a squash group folds back as its reword, and the commit gets the
    // typed message followed by the squash's.
    git(&dir, &["rebase", "--abort"]);
    let req = request_steps(&dir, RebaseBase::FromCommit(c2), |steps| {
        at(steps, 0).action = RebaseAction::Edit;
        at(steps, 1).action = RebaseAction::Reword;
        at(steps, 1).new_message = Some("Three, reworded".into());
        at(steps, 2).action = RebaseAction::Squash;
    });
    assert!(matches!(start_copy(&dir, &req), Outcome::Stopped { .. }));
    let r = remaining(&dir).unwrap();
    assert_eq!(kinds(&r.steps), ["reword c3", "squash c4"]);
    let head = r.steps[0].commit().unwrap();
    assert_eq!((head.new_message.as_deref(), head.combined_message.as_deref()), (Some("Three, reworded"), None));
    assert!(matches!(resume(&dir, Resume::Continue).unwrap(), Outcome::Finished { .. }));
    assert_eq!(message(&dir, "HEAD"), default_combined(&r.steps, 0));
    assert_eq!(message(&dir, "HEAD"), "Three, reworded\n\nc4");
    let _ = std::fs::remove_dir_all(&dir);
}

/// base, a and b changing f.txt, and c adding g.txt. A rebase from a that drops a, rewords b
/// and squashes c into it: b conflicts, with its group's squash and amend still to come.
fn reworded_conflict(name: &str) -> (PathBuf, Remaining) {
    let dir = scratch_repo(name);
    commit(&dir, "f.txt", "base\n", "base");
    let a = commit(&dir, "f.txt", "one\n", "a");
    commit(&dir, "f.txt", "two\n", "b");
    commit(&dir, "g.txt", "g\n", "c");
    let req = request(&dir, RebaseBase::FromCommit(a), |items| {
        items[0].action = RebaseAction::Drop;
        items[1].action = RebaseAction::Reword;
        items[1].new_message = Some("New b".into());
        items[2].action = RebaseAction::Squash;
    });
    match start_copy(&dir, &req) {
        Outcome::Stopped { progress, .. } => assert_eq!(progress.stop.kind, StopKind::Conflict),
        other => panic!("expected a conflict, got {other:?}"),
    }
    let r = remaining(&dir).unwrap();
    (dir, r)
}

#[test]
fn an_unfinished_reword_is_locked() {
    let (dir, r) = reworded_conflict("rebase-remaining-locked");
    assert_eq!(kinds(&r.locked), ["squash c"]);
    let lines: Vec<&str> = r.locked_text.lines().collect();
    assert_eq!(lines.len(), 2, "{}", r.locked_text);
    assert!(lines[0].starts_with("squash ") && lines[1].starts_with("exec h=$(git rev-parse"), "{}", r.locked_text);
    assert!(r.steps.is_empty());
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commands_the_list_cannot_show_are_refused() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-remaining-unsupported");
    for todo in [
        format!("edit {c2} c2\nlabel here\npick {c3} c3\npick {c4} c4\n"),
        format!("edit {c2} c2\nupdate-ref refs/heads/topic\npick {c3} c3\npick {c4} c4\n"),
        format!("edit {c2} c2\npick {c3} c3\nfixup -C {c4} c4\n"),
    ] {
        assert!(rebase_i(&dir, &c1.to_string(), &todo), "{todo}");
        assert_eq!(refused(remaining(&dir)), UNSUPPORTED_STEPS, "{todo}");
        git(&dir, &["rebase", "--abort"]);
        let _ = run(&dir, &args(&["update-ref", "-d", "refs/heads/topic"]));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_todo_reorders_drops_and_adds_steps() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-remaining-edit");
    assert!(rebase_i(&dir, &c1.to_string(), &format!("edit {c2} c2\npick {c3} c3\npick {c4} c4\n")));
    let r = remaining(&dir).unwrap();
    let mut steps = r.steps.clone();
    steps.swap(0, 1);
    steps[1].commit_mut().unwrap().action = RebaseAction::Drop;
    steps.push(TodoStep::Break);
    edit_todo(&dir, &edit_request(&r, steps), &SequenceEditor::Copy).unwrap();
    assert_eq!(state(&dir), RepositoryState::RebaseInteractive, "the rebase stays stopped");
    let after = remaining(&dir).unwrap();
    assert_eq!(kinds(&after.steps), ["pick c4", "drop c3", "break"]);
    match resume(&dir, Resume::Continue).unwrap() {
        Outcome::Stopped { progress, .. } => assert_eq!(progress.stop.kind, StopKind::Break),
        other => panic!("expected the break, got {other:?}"),
    }
    assert!(matches!(resume(&dir, Resume::Continue).unwrap(), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c2", "c1"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_todo_refuses_a_stale_or_different_list() {
    let (dir, [_, c2, ..]) = linear("rebase-remaining-stale");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| items[0].action = RebaseAction::Edit);
    assert!(matches!(start_copy(&dir, &req), Outcome::Stopped { .. }));
    let r = remaining(&dir).unwrap();

    // A different commit set.
    let mut fewer = r.steps.clone();
    fewer.pop();
    let e = edit_todo(&dir, &edit_request(&r, fewer), &SequenceEditor::Copy).unwrap_err();
    assert_eq!(e.to_string(), CHANGED);

    // The todo changed in between (edited in a terminal).
    let editor = "echo break >>";
    let (ok, out) = run_with_input(&dir, &args(&["rebase", "--edit-todo"]), "", &[("GIT_SEQUENCE_EDITOR", editor)]);
    assert!(ok, "{out}");
    let before = todo_text(&dir);
    assert_ne!(before, r.todo);
    let e = edit_todo(&dir, &edit_request(&r, Vec::new()), &SequenceEditor::Copy).unwrap_err();
    assert_eq!(e.to_string(), TODO_MOVED);
    assert_eq!(todo_text(&dir), before, "nothing changed");
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn locked_lines_are_kept_and_the_reword_lands() {
    let (dir, r) = reworded_conflict("rebase-remaining-locked-edit");
    let req = edit_request(&r, vec![TodoStep::Exec("git log -1 --format=%s > after.txt".into())]);
    edit_todo(&dir, &req, &SequenceEditor::Copy).unwrap();
    assert!(todo_text(&dir).starts_with(&r.locked_text), "{}", todo_text(&dir));
    write(&dir, "f.txt", "two\n");
    git(&dir, &["add", "f.txt"]);
    let outcome = resume(&dir, Resume::Continue).unwrap();
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["New b", "base"]);
    assert_eq!(message(&dir, "HEAD"), "New b\n\nc", "the squash's message follows the typed one");
    assert!(dir.join("g.txt").exists(), "the squash is in the reworded commit");
    assert_eq!(std::fs::read_to_string(dir.join("after.txt")).unwrap().trim(), "New b", "the exec runs after the amend");
    let _ = std::fs::remove_dir_all(&dir);
}

//! Reading where a rebase in progress stands, and resuming it with Continue or Skip.

use super::plan_tests::{commit, head, linear};
use super::run_tests::{log, request, start_copy, state};
use super::*;
use crate::error::Error;
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn stopped(outcome: Outcome) -> (bool, RebaseProgress) {
    match outcome {
        Outcome::Stopped { ok, progress, .. } => (ok, progress),
        other => panic!("expected a stop, got {other:?}"),
    }
}

fn resumed(dir: &Path, how: Resume) -> Outcome {
    resume(dir, how).unwrap()
}

#[test]
fn edits_report_steps() {
    let (dir, [_, c2, c3, _]) = linear("rebase-progress-edits");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        items[0].action = RebaseAction::Edit;
        items[1].action = RebaseAction::Edit;
    });
    let (ok, p) = stopped(start_copy(&dir, &req));
    assert!(ok);
    assert_eq!((p.step, p.total, p.stop.kind, p.stop.oid), (1, 3, StopKind::Edit, Some(c2)));
    assert_eq!(p.summary, "c2");
    assert!(p.interactive && !p.autostash);
    assert_eq!(progress(&dir), Some(p));

    let (ok, p) = stopped(resumed(&dir, Resume::Continue));
    assert!(ok);
    assert_eq!((p.step, p.total, p.stop.kind, p.stop.oid), (2, 3, StopKind::Edit, Some(c3)));
    let outcome = resumed(&dir, Resume::Continue);
    assert!(matches!(outcome, Outcome::Finished { stash_kept: false, .. }), "{outcome:?}");
    assert_eq!(progress(&dir), None);
    assert_eq!(log(&dir), ["c4", "c3", "c2", "c1"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reword_exec_lines_not_counted() {
    let (dir, [_, c2, _, c4]) = linear("rebase-progress-exec");
    let req = request(&dir, RebaseBase::FromCommit(c2), |items| {
        items[0].action = RebaseAction::Reword;
        items[0].new_message = Some("c2, reworded".into());
        items[2].action = RebaseAction::Edit;
    });
    let (_, p) = stopped(start_copy(&dir, &req));
    assert_eq!((p.step, p.total, p.stop.oid), (3, 3, Some(c4)), "only the commit rows count");
    let _ = std::fs::remove_dir_all(&dir);
}

/// base, then a and b both changing f.txt; a rebase from a that drops a and so conflicts on b.
fn conflicting(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    commit(&dir, "f.txt", "base\n", "base");
    let a = commit(&dir, "f.txt", "one\n", "a");
    let b = commit(&dir, "f.txt", "two\n", "b");
    let req = request(&dir, RebaseBase::FromCommit(a), |items| items[0].action = RebaseAction::Drop);
    let (ok, p) = stopped(start_copy(&dir, &req));
    assert!(!ok);
    assert_eq!((p.stop.kind, p.stop.oid, p.step, p.total), (StopKind::Conflict, Some(b), 2, 2));
    (dir, b)
}

#[test]
fn resume_conflict_paths() {
    let (dir, b) = conflicting("rebase-progress-conflict");
    let (ok, p) = stopped(resumed(&dir, Resume::Continue));
    assert!(!ok, "git refuses while the conflict is unresolved");
    assert_eq!((p.stop.kind, p.stop.oid), (StopKind::Conflict, Some(b)));
    write(&dir, "f.txt", "two\n");
    git(&dir, &["add", "f.txt"]);
    let outcome = resumed(&dir, Resume::Continue);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["b", "base"]);
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, _) = conflicting("rebase-progress-skip");
    let outcome = resumed(&dir, Resume::Skip);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["base"], "the conflicting commit is left out");
    assert_eq!(state(&dir), git2::RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_backend() {
    let dir = scratch_repo("rebase-progress-apply");
    commit(&dir, "f.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "f.txt", "other\n", "other");
    git(&dir, &["checkout", "-q", "main"]);
    let clean = crate::repo::load(&dir, &crate::repo::LoadOptions::all(10)).unwrap();
    assert_eq!(clean.rebase, None);
    assert_eq!(progress(&dir), None);
    assert!(matches!(resume(&dir, Resume::Continue), Err(Error::Unsupported(t)) if t == NOT_REBASING));

    commit(&dir, "f.txt", "mine\n", "mine");
    commit(&dir, "g.txt", "g\n", "g");
    let before = head(&dir);
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["rebase", "--apply", "other"]));
    assert!(!ok, "the first patch conflicts");
    let p = progress(&dir).expect("a rebase is in progress");
    assert_eq!((p.step, p.total, p.stop.kind, p.stop.oid), (1, 2, StopKind::Conflict, None));
    assert!(!p.interactive && p.summary.is_empty());
    let snap = crate::repo::load(&dir, &crate::repo::LoadOptions::all(10)).unwrap();
    assert_eq!(snap.rebase, Some(p));
    git(&dir, &["rebase", "--abort"]);
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_stops_tell_glowfly_amends_from_commands() {
    use super::remaining_tests::rebase_i;
    let (dir, [c1, c2, c3, c4]) = linear("rebase-progress-exec-kinds");
    // A failing amend of Glowfly's own (recognised by its message file's name).
    let own = format!("exec h=$(git rev-parse -q --verify HEAD); false /x/glowfly-rebase/reword-{c2}.txt");
    let todo = format!("pick {c2} c2\n{own}\npick {c3} c3\nexec echo hi && false\npick {c4} c4\nbreak\n");
    assert!(!rebase_i(&dir, &c1.to_string(), &todo));
    let p = progress(&dir).unwrap();
    assert_eq!((p.stop.kind, p.stop.oid, p.command.clone()), (StopKind::Exec, Some(c2), None));

    let (ok, p) = stopped(resumed(&dir, Resume::Continue));
    assert!(!ok);
    assert_eq!((p.stop.kind, p.stop.oid), (StopKind::Command, Some(c3)));
    assert_eq!(p.command.as_deref(), Some("echo hi && false"));

    let (ok, p) = stopped(resumed(&dir, Resume::Continue));
    assert!(ok);
    assert_eq!((p.stop.kind, p.stop.oid, p.command), (StopKind::Break, Some(c4), None));
    assert!(matches!(resumed(&dir, Resume::Continue), Outcome::Finished { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflicted_merge_stops_at_the_merge() {
    let dir = scratch_repo("rebase-progress-merge-conflict");
    git(&dir, &["config", "rerere.enabled", "false"]);
    commit(&dir, "f.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "f.txt", "side\n", "on side");
    git(&dir, &["checkout", "-q", "main"]);
    let mine = commit(&dir, "f.txt", "main\n", "on main");
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "-q", "side", "-m", "Merge side"]));
    assert!(!ok, "the merge conflicts");
    write(&dir, "f.txt", "resolved\n");
    git(&dir, &["commit", "-q", "-am", "Merge side"]);
    let merge = head(&dir);
    // Rewording the commit before the merge makes git redo the merge, which conflicts again.
    let req = request_steps_of(&dir, mine);
    let (ok, p) = stopped(start_copy(&dir, &req));
    assert!(!ok);
    assert_eq!((p.stop.kind, p.stop.oid), (StopKind::Conflict, Some(merge)));
    assert_eq!((p.step, p.total), (3, 3), "the merge line counts as a step");
    assert_eq!(p.summary, "Merge side");
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The list from `from` with that commit reworded.
fn request_steps_of(dir: &Path, from: Oid) -> StartRequest {
    let p = plan(dir, &RebaseBase::FromCommit(from)).unwrap();
    let mut steps = p.steps;
    let row = steps.iter().position(|s| s.oid() == Some(from)).unwrap();
    let item = steps[row].commit_mut().unwrap();
    item.action = RebaseAction::Reword;
    item.new_message = Some("on main, reworded".into());
    StartRequest { head: p.head, upstream: p.upstream, items: steps, autostash: false }
}

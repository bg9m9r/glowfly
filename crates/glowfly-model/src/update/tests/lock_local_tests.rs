//! Try again and Delete lock file for a working-tree op that failed on a lock file
//! (`LockMsg::LocalLocked`, update/lock_local.rs): the note and toast every retryable op gets,
//! how each op is sent again, a folder stage staying a folder stage, the stale lock's Delete
//! lock file and its wording, the trust prompt, and the conflict changes that are never offered.

use super::file_trust_tests::{clean, files, held, send};
use super::update;
use crate::effect::{Effect, FilesEffect, LocalOp, LockEffect};
use crate::logs::LogsMsg;
use crate::msg::{LockMsg, Msg};
use crate::state::lock_retry::{LocalRetry, LockRetry};
use crate::state::logs::EntryId;
use crate::state::op_fixture::rows;
use crate::state::{AppState, Modal};
use glowfly_core::error::WriteFailed;
use glowfly_core::lock_file::{Found, Refusal};
use glowfly_core::repo::{DiffLine, Hunk, HunkOp};
use std::path::PathBuf;
use std::time::Instant;

const SAID: &str = "the index is locked; this might be due to a concurrent or crashed process";
const SENTENCE: &str = "Another Git program is using this repository (index.lock exists). Wait for it to finish, then try again. If none is running, delete that file.";

/// The lock's path, as text only: nothing here touches it.
fn lock_of(st: &AppState) -> String {
    st.workdir().unwrap().join(".git/index.lock").to_string_lossy().into_owned()
}

fn hunk() -> Hunk {
    let line = DiffLine { origin: '+', old: None, new: Some(1), text: "b".into() };
    Hunk { header: "@@ -1 +1,2 @@".into(), lines: vec![line] }
}

fn lines(op: HunkOp) -> LocalOp {
    LocalOp::Lines { op, path: "a.txt".into(), hunk: 0, expected: hunk(), lines: vec![0], ignore_whitespace: true }
}

fn hunk_op(op: HunkOp, ignore_whitespace: bool) -> LocalOp {
    LocalOp::Hunk { op, path: "a.txt".into(), index: 0, expected: hunk(), ignore_whitespace }
}

/// The ops Try again is offered for.
fn retryable() -> Vec<LocalOp> {
    let one = || vec!["a.txt".to_owned()];
    let mut ops = vec![
        LocalOp::StageFile("a.txt".into()),
        LocalOp::StagePaths(one()),
        LocalOp::StageAll,
        LocalOp::UnstagePaths(one()),
        LocalOp::DiscardFile("a.txt".into()),
    ];
    for op in [HunkOp::Stage, HunkOp::Unstage, HunkOp::Discard] {
        ops.push(hunk_op(op, false));
        ops.push(lines(op));
    }
    ops
}

fn locked(st: &AppState, op: LocalOp) -> Msg {
    let error = WriteFailed::Locked { lock: lock_of(st), said: SAID.into() };
    Msg::Lock(LockMsg::LocalLocked { workdir: st.workdir().unwrap(), op: Box::new(op), lock: lock_of(st), error })
}

/// The entry of the one lock check `fx` asks for.
fn check_entry(fx: &[Effect]) -> EntryId {
    let mut checks = fx.iter().filter_map(|e| match e {
        Effect::Lock(LockEffect::Check { entry, .. }) => Some(*entry),
        _ => None,
    });
    let entry = checks.next().expect("a lock check");
    assert!(checks.next().is_none(), "one check only");
    entry
}

fn again(st: &AppState, op: LocalOp) -> Msg {
    let tab = st.tabs[st.active_tab].id;
    Msg::Lock(LockMsg::Again(Box::new(LocalRetry { tab, workdir: st.workdir().unwrap(), op })))
}

/// The labels of the newest toast's buttons, in order.
fn buttons(st: &AppState) -> Vec<String> {
    let t = st.toasts.last().expect("a toast");
    [&t.action, &t.alt, &t.details].into_iter().flatten().map(|a| a.label.clone()).collect()
}

/// The local ops among `fx`.
fn ops(fx: Vec<Effect>) -> Vec<LocalOp> {
    files(fx)
        .into_iter()
        .filter_map(|(e, _)| match e {
            FilesEffect::LocalOp { op, .. } => Some(op),
            _ => None,
        })
        .collect()
}

#[test]
fn each_retryable_op_gets_try_again_and_a_lock_check() {
    for (i, op) in retryable().into_iter().enumerate() {
        let mut st = clean(&format!("ll-each-{i}"));
        let log_before = st.log.len();
        let msg = locked(&st, op.clone());
        let fx = update(&mut st, msg, Instant::now());
        let entry = check_entry(&fx);
        assert!(fx.len() > 1, "{op:?}: the status is read again too");
        let note = st.log.iter().find(|e| e.id == entry).expect("the note");
        assert!(!note.ok);
        let mut text = note.text.lines();
        assert_eq!((text.next(), text.next()), (Some(SENTENCE), Some(SAID)), "{op:?}");
        assert!(st.logs.open.contains(&entry), "folded open");
        let fix = st.logs.fix(entry).expect("Try again is kept");
        assert_eq!(fix.label, "Try again");
        let Msg::Lock(LockMsg::Again(l)) = &*fix.msg else { panic!("not Again") };
        assert_eq!((&l.op, &l.workdir, l.tab), (&op, &st.workdir().unwrap(), st.tabs[st.active_tab].id));
        assert_eq!(buttons(&st), ["Try again", "Details"], "{op:?}");
        assert_eq!(st.log.len(), log_before + 1, "the toast adds no second line");
    }
}

#[test]
fn try_again_sends_each_op_the_way_it_was_first_sent() {
    for (i, op) in retryable().into_iter().enumerate() {
        let mut st = clean(&format!("ll-send-{i}"));
        st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M')]);
        st.snap.as_mut().unwrap().staged = rows(&[("a.txt", 'M')]);
        let generation = st.session.diff.generation;
        let msg = again(&st, op.clone());
        let sent = ops(send(&mut st, msg));
        assert_eq!(sent, std::slice::from_ref(&op), "{op:?}");
        let stale_hunks = matches!(op, LocalOp::Hunk { .. } | LocalOp::Lines { .. });
        assert_eq!(st.session.diff.generation, generation + u64::from(stale_hunks), "{op:?}");
    }
}

#[test]
fn a_folder_stage_is_tried_again_as_a_folder() {
    let mut st = clean("ll-folder");
    st.snap.as_mut().unwrap().unstaged = rows(&[("src/a.txt", '?')]);
    let msg = again(&st, LocalOp::StageFile("src".into()));
    assert_eq!(ops(send(&mut st, msg)), [LocalOp::StageFile("src".into())]);
    st.snap.as_mut().unwrap().unstaged = rows(&[("src/a.txt", '!')]);
    let toasts = st.toasts.len();
    let msg = again(&st, LocalOp::StageFile("src".into()));
    assert!(ops(send(&mut st, msg)).is_empty(), "a conflicted file under it refuses the stage");
    assert_eq!(st.toasts.len(), toasts + 1, "a toast says why");
}

#[test]
fn a_stale_lock_puts_delete_first_and_asks_with_the_change_wording() {
    let op = hunk_op(HunkOp::Stage, false);
    let mut st = clean("ll-stale");
    let msg = locked(&st, op.clone());
    let entry = check_entry(&update(&mut st, msg, Instant::now()));
    let (workdir, lock) = (st.workdir().unwrap(), lock_of(&st));
    let stale = Ok(Found::Stale(PathBuf::from(&lock)));
    update(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir, lock: lock.clone(), result: stale }), Instant::now());
    assert_eq!(st.logs.fix(entry).map(|f| f.label.as_str()), Some("Delete lock file"));
    assert_eq!(st.logs.alt_fix(entry).map(|f| f.label.as_str()), Some("Try again"));
    assert_eq!(buttons(&st), ["Delete lock file", "Try again", "Details"]);

    update(&mut st, Msg::Logs(LogsMsg::Fix(entry)), Instant::now());
    let Some(Modal::ConfirmDanger { title, body, .. }) = &st.modal else { panic!("it asks first") };
    assert_eq!(title, "Delete the lock file?");
    assert!(body.contains("make the change again"), "{body}");
    assert!(body.contains("index.lock"), "{body}");
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    let retry = fx.into_iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Delete { retry, .. }) => Some(retry),
        _ => None,
    });
    assert!(matches!(retry, Some(LockRetry::Local(l)) if l.op == op), "the delete carries the op");

    // Gone already, or git running: only Try again.
    for (i, result) in [Ok(Found::Gone), Err(Refusal::GitRunning(vec![(7, "git".into())]))].into_iter().enumerate() {
        let mut st = clean(&format!("ll-not-stale-{i}"));
        let msg = locked(&st, op.clone());
        let entry = check_entry(&update(&mut st, msg, Instant::now()));
        let (workdir, lock) = (st.workdir().unwrap(), lock_of(&st));
        update(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir, lock, result }), Instant::now());
        assert_eq!(buttons(&st), ["Try again", "Details"]);
        assert!(st.logs.alt_fix(entry).is_none());
    }
}

#[test]
fn a_retried_stage_waits_for_trust_and_a_hunk_does_not() {
    let mut st = held("ll-held");
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M')]);
    let msg = again(&st, LocalOp::StageFile("a.txt".into()));
    assert!(files(send(&mut st, msg)).is_empty(), "nothing runs before the decision");
    let Some(Modal::Trust(p)) = &st.modal else { panic!("the trust prompt") };
    assert_eq!(p.held.as_deref(), Some("Stage"));
    let mut st = held("ll-held-hunk");
    let op = hunk_op(HunkOp::Stage, false);
    let msg = again(&st, op.clone());
    assert_eq!(ops(send(&mut st, msg)), [op], "libgit2 only: no trust needed");
}

#[test]
fn conflict_changes_are_never_tried_again() {
    use glowfly_core::repo::{ConflictIds, Side};
    let ids = ConflictIds { ancestor: None, ours: None, theirs: None };
    let never = [
        LocalOp::ResolveConflict { path: "a.txt".into(), contents: String::new(), expected: ids.clone() },
        LocalOp::SavePartialResolution { path: "a.txt".into(), contents: String::new(), expected: ids.clone() },
        LocalOp::ResolveWithSide { path: "a.txt".into(), side: Side::Ours, expected: ids },
        LocalOp::MarkResolved(vec!["a.txt".into()]),
        LocalOp::MarkConflicted(vec!["a.txt".into()]),
        LocalOp::TakeTrivialPicks(vec!["a.txt".into()]),
    ];
    for op in never {
        assert!(!op.retryable(), "{op:?}");
        let mut st = clean("ll-never");
        let msg = again(&st, op);
        assert!(files(send(&mut st, msg)).is_empty());
    }
    assert!(retryable().iter().all(LocalOp::retryable));
}

#[test]
fn cancelling_delete_lock_file_keeps_both_fixes_and_the_toast() {
    use crate::msg::{DialogMsg, Dismiss};
    let op = hunk_op(HunkOp::Stage, false);
    let mut st = clean("ll-cancel");
    let msg = locked(&st, op.clone());
    let entry = check_entry(&update(&mut st, msg, Instant::now()));
    let (workdir, lock) = (st.workdir().unwrap(), lock_of(&st));
    let stale = Ok(Found::Stale(PathBuf::from(&lock)));
    update(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir, lock, result: stale }), Instant::now());
    let kept = |st: &AppState| {
        assert_eq!(st.logs.fix(entry).map(|f| f.label.as_str()), Some("Delete lock file"));
        assert_eq!(st.logs.alt_fix(entry).map(|f| f.label.as_str()), Some("Try again"));
        assert_eq!(buttons(st), ["Delete lock file", "Try again", "Details"]);
    };

    // The panel's button, then Cancel.
    update(&mut st, Msg::Logs(LogsMsg::Fix(entry)), Instant::now());
    let Some(Modal::ConfirmDanger { title, .. }) = &st.modal else { panic!("it asks first") };
    assert_eq!(title, "Delete the lock file?");
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.modal.is_none());
    kept(&st);

    // The toast's button, then Esc: the toast keeps its id.
    let id = st.toasts.last().unwrap().id;
    update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })));
    assert!(st.toasts.iter().any(|t| t.id == id), "the toast stays while it asks");
    update(&mut st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Escape)), Instant::now());
    assert!(st.modal.is_none());
    assert_eq!(st.toasts.last().unwrap().id, id);
    kept(&st);

    // Confirming takes both fixes and the toast, and deletes.
    update(&mut st, Msg::ToastAction(id), Instant::now());
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    let retry = fx.into_iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Delete { retry, .. }) => Some(retry),
        _ => None,
    });
    assert!(matches!(retry, Some(LockRetry::Local(l)) if l.op == op), "the delete carries the op");
    assert!(st.logs.fix(entry).is_none() && st.logs.alt_fix(entry).is_none());
    assert!(st.toasts.iter().all(|t| t.id != id));
}

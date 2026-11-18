//! Try again and Delete lock file for working-tree ops that met a stale `index.lock`: a hunk
//! staged through libgit2 offers Try again, then Delete lock file once the check finds the lock
//! stale, asks first, deletes and stages the hunk again; Try again alone re-sends the hunk or
//! the whole-file stage the way it was first sent; and every refusal says why.

use super::auto_stash_tests::up;
use super::tests::{drive, finish, git, scratch_repo, state};
use glowfly_core::lock_file::{Found, Refusal};
use glowfly_core::paths::resolve;
use glowfly_core::repo::{DiffTarget, HunkOp};
use glowfly_model::effect::{ConflictEffect, Effect, FilesEffect, LocalOp, LockEffect};
use glowfly_model::msg::{LockMsg, Msg, RepoMsg, StageMsg};
use glowfly_model::state::lock_retry::{LocalRetry, LockRetry};
use glowfly_model::state::logs::EntryId;
use glowfly_model::state::tabs::TabId;
use glowfly_model::state::toasts::Toast;
use glowfly_model::state::{AppState, Modal};
use std::path::{Path, PathBuf};

/// The scratch repository opened, with a.txt's unstaged diff shown.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    (st, dir)
}

/// Leaves a stale lock beside the index.
fn lock(dir: &Path) -> PathBuf {
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    lock
}

fn last_toast(st: &AppState) -> &Toast {
    st.toasts.last().expect("a toast")
}

/// The labels of a toast's buttons, in order.
fn buttons(t: &Toast) -> Vec<&str> {
    [&t.action, &t.alt, &t.details].into_iter().flatten().map(|a| a.label.as_str()).collect()
}

/// The working-tree op among `fx`.
fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::Files(FilesEffect::LocalOp { op, .. }) => Some(op),
        _ => None,
    })
}

/// Sends `msg`, runs only its working-tree op through the runner (it meets the lock) and hands
/// the result to `update`. Returns the effects that follows with, and the lock check asked for.
fn fail(st: &mut AppState, msg: Msg) -> (Vec<Effect>, EntryId, String) {
    let fx = up(st, msg);
    let op = fx.into_iter().find(|e| matches!(e, Effect::Files(FilesEffect::LocalOp { .. }))).expect("the op is sent");
    let result = glowfly_runner::testing::execute(op).expect("a result");
    let Msg::Scoped { msg, .. } = &result else { panic!("scoped to its repository") };
    assert!(matches!(**msg, Msg::Lock(LockMsg::LocalLocked { .. })), "the op failed on the lock");
    let fx = up(st, result);
    let check = fx.iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Check { lock, entry, .. }) => Some((*entry, lock.clone())),
        _ => None,
    });
    let (entry, lock) = check.expect("the lock is checked");
    // Everything else (the status read after it) runs; the check would scan processes.
    let rest = fx.into_iter().filter(|e| !matches!(e, Effect::Lock(_))).collect();
    (rest, entry, lock)
}

fn stage_hunk(st: &AppState) -> Msg {
    hunk(st, HunkOp::Stage)
}

fn hunk(st: &AppState, op: HunkOp) -> Msg {
    Msg::Stage(StageMsg::Hunk { op, path: "a.txt".into(), index: 0, generation: st.hunk_ops_gen().expect("the diff is shown") })
}

fn staged(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.staged.iter().any(|f| f.path == path))
}

#[test]
fn a_stale_lock_is_deleted_and_the_hunk_staged_again() {
    let (mut st, dir) = opened("ll-hunk");
    let lock_file = lock(&dir);
    let toasts_before = st.toasts.len();
    let msg = stage_hunk(&st);
    let (rest, entry, named) = fail(&mut st, msg);
    let t = last_toast(&st);
    assert!(st.toasts.len() > toasts_before);
    assert!(t.text.starts_with("Another Git program is using this repository"), "{}", t.text);
    assert_eq!(buttons(t), ["Try again", "Details"]);
    assert!(st.logs.open.contains(&entry), "the note is folded open");
    let note = st.log.iter().find(|e| e.id == entry).expect("the note");
    assert!(note.text.lines().nth(1).is_some_and(|l| l.starts_with("the index is locked")), "{}", note.text);
    assert_eq!(resolve(Path::new(&named)), resolve(&lock_file), "the index's own lock is checked");
    finish(&mut st, rest);

    // The runner found it stale and no git running.
    let workdir = st.workdir().unwrap();
    let stale = Ok(Found::Stale(PathBuf::from(&named)));
    up(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir, lock: named.clone(), result: stale }));
    let t = last_toast(&st);
    assert_eq!(buttons(t), ["Delete lock file", "Try again", "Details"]);
    assert_eq!(st.logs.fix(entry).map(|f| f.label.as_str()), Some("Delete lock file"));
    assert_eq!(st.logs.alt_fix(entry).map(|f| f.label.as_str()), Some("Try again"), "the panel offers both");

    let id = t.id;
    up(&mut st, Msg::ToastAction(id));
    let Some(Modal::ConfirmDanger { title, .. }) = &st.modal else { panic!("it asks first") };
    assert_eq!(title, "Delete the lock file?");
    let fx = up(&mut st, Msg::ConfirmAccepted);
    let delete = fx.into_iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Delete { lock, retry, .. }) => Some((lock, retry)),
        _ => None,
    });
    let (at, retry) = delete.expect("the delete");
    assert!(matches!(&retry, LockRetry::Local(l) if matches!(l.op, LocalOp::Hunk { op: HunkOp::Stage, .. })));
    assert!(lock_file.exists(), "nothing is deleted before the runner checks again");

    // The runner's delete.
    std::fs::remove_file(&lock_file).unwrap();
    let fx = up(&mut st, Msg::Lock(LockMsg::Deleted { lock: at, retry, result: Ok(Found::Stale(lock_file.clone())) }));
    assert!(st.log.iter().any(|e| e.text == "Deleted index.lock."), "the delete is logged");
    assert!(matches!(local_op(&fx), Some(LocalOp::Hunk { op: HunkOp::Stage, .. })), "then the hunk is staged again");
    finish(&mut st, fx);
    assert_eq!(st.hunk_ops_gen(), None, "its only hunk is staged: no unstaged diff is left to apply hunks to");
    assert!(staged(&st, "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn try_again_sends_the_hunk_or_the_whole_file_stage_again() {
    let (mut st, dir) = opened("ll-again");
    let lock_file = lock(&dir);
    let msg = stage_hunk(&st);
    let (rest, _, _) = fail(&mut st, msg);
    finish(&mut st, rest);
    std::fs::remove_file(&lock_file).unwrap();
    let generation = st.session.diff.generation;
    let id = last_toast(&st).id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert!(matches!(local_op(&fx), Some(LocalOp::Hunk { op: HunkOp::Stage, index: 0, .. })), "the same hunk op");
    assert_eq!(st.session.diff.generation, generation + 1, "its hunks are stale until the reload");
    assert!(st.logs.fixes.is_empty(), "Try again is offered once");

    // A whole-file stage runs git, whose lock failure gets the same toast.
    lock(&dir);
    let (rest, _, _) = fail(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    let t = last_toast(&st);
    assert!(t.text.starts_with("Another Git program is using this repository"), "{}", t.text);
    assert_eq!(buttons(t), ["Try again", "Details"]);
    finish(&mut st, rest);
    std::fs::remove_file(&lock_file).unwrap();
    let id = last_toast(&st).id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("a.txt".into())), "through StageMsg::StageFile");

    // A file conflicted since then goes to the markers check, as StageFile sends it.
    lock(&dir);
    let (rest, _, _) = fail(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    finish(&mut st, rest);
    std::fs::remove_file(&lock_file).unwrap();
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.iter_mut().filter(|f| f.path == "a.txt").for_each(|f| f.status = '!');
    let id = last_toast(&st).id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(local_op(&fx), None, "nothing is staged");
    assert!(fx.iter().any(|e| matches!(e, Effect::Conflict(ConflictEffect::ReadFiles { .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn try_again_and_the_delete_are_refused_with_reasons() {
    let (mut st, dir) = opened("ll-refused");
    let workdir = st.workdir().unwrap();
    let op = LocalOp::StageFile("a.txt".into());
    let closed = LocalRetry { tab: TabId::next(), workdir: workdir.clone(), op: op.clone() };
    let fx = up(&mut st, Msg::Lock(LockMsg::Again(Box::new(closed))));
    assert_eq!(local_op(&fx), None);
    assert_eq!(last_toast(&st).text, "The change wasn't made again: its tab was closed.");
    let tab = st.tabs[st.active_tab].id;
    let moved = LocalRetry { tab, workdir: dir.join("elsewhere"), op: op.clone() };
    let fx = up(&mut st, Msg::Lock(LockMsg::Again(Box::new(moved))));
    assert_eq!(local_op(&fx), None);
    assert_eq!(last_toast(&st).text, "The change wasn't made again: its tab shows another repository now.");

    // Asked while a job of Glowfly's own runs: refused before the runner is asked.
    let lock_file = lock(&dir);
    let named = lock_file.to_string_lossy().replace('\\', "/");
    let retry = || LockRetry::Local(Box::new(LocalRetry { tab, workdir: workdir.clone(), op: op.clone() }));
    up(&mut st, Msg::git("Push", &["push"]));
    assert!(st.job.is_some());
    let fx = up(&mut st, Msg::Lock(LockMsg::Delete { workdir: workdir.clone(), lock: named.clone(), retry: retry() }));
    assert!(fx.is_empty());
    assert_eq!(last_toast(&st).text, "index.lock wasn't deleted: Push is still running.");

    // The runner's own check found git running: only Try again stays.
    let running = Err(Refusal::GitRunning(vec![(42, "git.exe".into())]));
    up(&mut st, Msg::Lock(LockMsg::Deleted { lock: named, retry: retry(), result: running }));
    let t = last_toast(&st);
    assert!(t.text.starts_with("index.lock wasn't deleted:"), "{}", t.text);
    assert_eq!(buttons(t), ["Try again"]);
    assert!(matches!(t.action.as_ref().map(|a| &*a.msg), Some(Msg::Lock(LockMsg::Again(_)))));
    assert!(lock_file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// a.txt's text with line ends as `\n`, whatever the checkout's settings made them.
fn text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n")
}

#[test]
fn an_unstage_and_a_hunk_discard_run_again_once_the_lock_is_gone() {
    let dir = scratch_repo("ll-unstage");
    git(&dir, &["add", "a.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let lock_file = lock(&dir);
    let (rest, _, _) = fail(&mut st, Msg::Stage(StageMsg::UnstageFile("a.txt".into())));
    finish(&mut st, rest);
    assert_eq!(buttons(last_toast(&st)), ["Try again", "Details"]);
    assert!(staged(&st, "a.txt"), "nothing was unstaged");
    std::fs::remove_file(&lock_file).unwrap();
    let id = last_toast(&st).id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["a.txt".into()])));
    finish(&mut st, fx);
    assert!(!staged(&st, "a.txt"), "the file is unstaged");

    // The same file's hunk discarded while the lock is back.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    let lock_file = lock(&dir);
    let msg = hunk(&st, HunkOp::Discard);
    let (rest, _, _) = fail(&mut st, msg);
    finish(&mut st, rest);
    assert_eq!(buttons(last_toast(&st)), ["Try again", "Details"]);
    assert_eq!(text(&dir), "one\ntwo\n", "nothing was discarded");
    assert!(st.session.undo.undo.is_empty());
    std::fs::remove_file(&lock_file).unwrap();
    let id = last_toast(&st).id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert!(matches!(local_op(&fx), Some(LocalOp::Hunk { op: HunkOp::Discard, .. })));
    finish(&mut st, fx);
    assert_eq!(text(&dir), "one\n", "the hunk is discarded");
    assert_eq!(st.session.undo.undo.len(), 1, "and can be undone");
    let _ = std::fs::remove_dir_all(&dir);
}

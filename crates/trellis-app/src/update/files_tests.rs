//! Headless tests for the file actions: the restore confirmation and its refusals, the effect
//! and result handling, and opening or showing working-tree files.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::FilesMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::history::FileHistoryView;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Modal, Selection};
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, Comparison, FileEntry, LoadOptions, RestoredFrom};
use trellis_core::undo::Action;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A loaded scratch repository (one commit; a.txt modified in the working tree).
fn loaded(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

fn ask(oid: Oid, path: &str, deleted: bool) -> Msg {
    Msg::Files(FilesMsg::RestoreFile { oid, path: path.into(), deleted, confirmed: false, overwrite: false })
}

fn confirm_body(st: &AppState) -> &str {
    match &st.modal {
        Some(Modal::Confirm { body, .. }) => body,
        _ => panic!("expected the confirmation"),
    }
}

#[test]
fn restore_asks_first() {
    let (mut st, dir) = loaded("files-ask");
    st.snap.as_mut().unwrap().unstaged.clear();
    let oid = head(&dir);
    assert!(run(&mut st, ask(oid, "a.txt", false)).is_empty());
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("expected the confirmation") };
    assert_eq!(title, "Restore file");
    assert_eq!(button, "Restore");
    assert!(body.contains(&short(oid)) && body.contains("left unstaged"), "{body}");
    assert!(!body.contains("overwritten"));
    assert!(
        matches!(&**then, Msg::Files(FilesMsg::RestoreFile { oid: o, path, deleted: false, confirmed: true, overwrite: false }) if *o == oid && path == "a.txt")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_warns_about_local_changes() {
    let (mut st, dir) = loaded("files-warn");
    let oid = head(&dir);
    run(&mut st, ask(oid, "a.txt", false));
    assert!(confirm_body(&st).contains("overwritten"), "{}", confirm_body(&st));
    assert!(confirm_body(&st).contains("Undo"), "the overwritten edits can come back: {}", confirm_body(&st));
    run(&mut st, Msg::CloseModal);

    run(&mut st, ask(oid, "a.txt", true));
    assert!(confirm_body(&st).contains("before commit"), "{}", confirm_body(&st));

    // An untracked file at the path counts as uncommitted changes too.
    st.snap.as_mut().unwrap().unstaged = vec![FileEntry { path: "new.txt".into(), status: '?' }];
    run(&mut st, ask(oid, "new.txt", true));
    assert!(confirm_body(&st).contains("overwritten"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_refuses_conflicted() {
    let (mut st, dir) = loaded("files-conflict");
    st.snap.as_mut().unwrap().unstaged = vec![FileEntry { path: "a.txt".into(), status: '!' }];
    assert!(run(&mut st, ask(head(&dir), "a.txt", false)).is_empty());
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("conflict") && l.text.contains("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirmed_restore_sends_effect() {
    let (mut st, dir) = loaded("files-confirm");
    let oid = head(&dir);
    run(&mut st, ask(oid, "a.txt", false));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(
        matches!(&fx[..], [Effect::RestoreFile { workdir, oid: o, path, overwrite: true }] if *workdir == dir && *o == oid && path == "a.txt"),
        "{fx:?}"
    );
    assert_eq!(fx[0].scope(), Some(&dir));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restored_toasts_and_refreshes() {
    let (mut st, dir) = loaded("files-restored");
    let oid = head(&dir);
    let mut detail = repo::commit_detail(&dir, oid).unwrap();
    detail.absent.insert("a.txt".into());
    st.detail = Some(detail);

    let fx = run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Commit), undo: None }));
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok && toast.text.contains("a.txt") && toast.text.contains(&short(oid)), "{}", toast.text);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    assert!(st.detail.as_ref().unwrap().in_workdir("a.txt"), "Open and Show become available");
    finish(&mut st, fx);

    let fx =
        run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Parent(oid)), undo: None }));
    finish(&mut st, fx);
    assert!(st.toasts.last().unwrap().text.contains("before"));

    let fx = run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Err("boom".into()), undo: None }));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not restore a.txt: boom"));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_marks_history_file_on_disk() {
    let (mut st, dir) = loaded("files-restored-history");
    let oid = head(&dir);
    st.history = Some(FileHistoryView::loading("a.txt".into(), None));
    let on_disk = |st: &AppState| st.history.as_ref().unwrap().on_disk;

    run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "old.txt".into(), oid, result: Ok(RestoredFrom::Commit), undo: None }));
    assert!(!on_disk(&st), "another path");
    run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Err("boom".into()), undo: None }));
    assert!(!on_disk(&st), "a failed restore");
    run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Commit), undo: None }));
    assert!(on_disk(&st), "Open and Show become available in the history");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scoped_error_for_inactive_tab_is_logged() {
    let (mut st, dir) = loaded("files-scoped");
    let msg = Msg::Files(FilesMsg::FileRestored { path: "b.txt".into(), oid: head(&dir), result: Err("in the way".into()), undo: None });
    let fx = run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) });
    assert!(fx.is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not restore b.txt: in the way"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_restore() {
    let dir = scratch_repo("files-e2e");
    let first = head(&dir);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["commit", "-q", "-am", "second"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.as_ref().unwrap().unstaged.is_empty());
    drive(
        &mut st,
        Msg::Files(FilesMsg::RestoreFile { oid: first, path: "a.txt".into(), deleted: false, confirmed: true, overwrite: false }),
    );
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"), "shows as an unstaged change");
    assert!(st.snap.as_ref().unwrap().staged.is_empty(), "nothing staged");
    assert!(st.toasts.last().is_some_and(|t| t.ok));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comparison_restore_clears_absent() {
    let (mut st, dir) = loaded("files-compare-absent");
    let oid = head(&dir);
    let files = vec![FileEntry { path: "a.txt".into(), status: 'D' }];
    let changes = Comparison { files, absent: ["a.txt".to_owned()].into() };
    let other = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    st.selection = Selection::Commits(MultiSelection {
        oids: vec![oid, other],
        anchor: oid,
        changes: Some(Ok(changes)),
        pick_blocked: None,
        export_blocked: None,
    });

    let fx = run(&mut st, Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Commit), undo: None }));
    let Selection::Commits(m) = &st.selection else { panic!("still two commits") };
    let c = m.changes.as_ref().unwrap().as_ref().unwrap();
    assert!(c.in_workdir("a.txt"), "Open and Show become available");
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.contains(&format!("Restored a.txt from {}", short(oid))), "{}", toast.text);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file the newer commit deleted comes back from the older commit of the pair.
#[test]
fn end_to_end_restore_from_comparison() {
    let dir = scratch_repo("files-compare-e2e");
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    std::fs::write(
        dir.join("gone.txt"),
        "v1
",
    )
    .unwrap();
    git(&dir, &["add", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "add gone"]);
    let older = head(&dir);
    git(&dir, &["rm", "-q", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "delete gone"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));

    drive(
        &mut st,
        Msg::Files(FilesMsg::RestoreFile { oid: older, path: "gone.txt".into(), deleted: false, confirmed: true, overwrite: false }),
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("gone.txt")).unwrap(),
        "v1
"
    );
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "gone.txt" && f.status == '?'), "{:?}", snap.unstaged);
    assert!(snap.staged.is_empty(), "nothing staged");
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok && toast.text.contains(&short(older)), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_and_show_build_absolute_paths() {
    let mut st = state();
    assert!(run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile("src/a.txt".into()))).is_empty(), "no repository open");
    assert!(run(&mut st, Msg::Files(FilesMsg::ShowWorkingFile("src/a.txt".into()))).is_empty());

    let (mut st, dir) = loaded("files-open");
    let fx = run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile("src/a.txt".into())));
    assert!(matches!(&fx[..], [Effect::OpenPath(p)] if *p == dir.join("src/a.txt")), "{fx:?}");
    let fx = run(&mut st, Msg::Files(FilesMsg::ShowWorkingFile("src/a.txt".into())));
    assert!(matches!(&fx[..], [Effect::RevealPath(p)] if *p == dir.join("src/a.txt")), "{fx:?}");
    assert_eq!(fx[0].scope(), None, "not tied to a tab");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignored_file_is_not_overwritten_without_a_warning() {
    // .env was committed once, then untracked and ignored; the local copy holds secrets.
    // Status never lists it, so the first confirmation can't warn, and core must catch it.
    let dir = scratch_repo("files-ignored");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join(".env"), "TOKEN=old\n").unwrap();
    git(&dir, &["add", ".env"]);
    git(&dir, &["commit", "-q", "-m", "env"]);
    let old = head(&dir);
    git(&dir, &["rm", "-q", "--cached", ".env"]);
    std::fs::write(dir.join(".gitignore"), ".env\n").unwrap();
    git(&dir, &["add", ".gitignore"]);
    git(&dir, &["commit", "-q", "-m", "ignore env"]);
    std::fs::write(dir.join(".env"), "TOKEN=secret\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));

    drive(&mut st, ask(old, ".env", false));
    assert!(!confirm_body(&st).contains("overwritten"), "status shows nothing to warn about");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(std::fs::read_to_string(dir.join(".env")).unwrap(), "TOKEN=secret\n", "nothing written");
    let Some(Modal::Confirm { body, then, .. }) = &st.modal else { panic!("asks again") };
    assert!(body.contains("ignored") && body.contains("Undo") && body.contains(&short(old)), "{body}");
    assert!(matches!(&**then, Msg::Files(FilesMsg::RestoreFile { confirmed: true, overwrite: true, .. })));

    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(std::fs::read_to_string(dir.join(".env")).unwrap(), "TOKEN=old\n", "written once the user agreed");
    assert!(st.toasts.last().is_some_and(|t| t.ok));
    assert!(matches!(st.undo.undo.last(), Some(Action::Restore { path, .. }) if path == ".env"), "{:?}", st.undo.undo);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join(".env")).unwrap(), "TOKEN=secret\n", "undo brings the secret back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn would_overwrite_for_inactive_tab_is_logged() {
    let (mut st, dir) = loaded("files-scoped-overwrite");
    let msg = Msg::Files(FilesMsg::RestoreWouldOverwrite { path: ".env".into(), oid: head(&dir), reason: ".env is ignored by git".into() });
    run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) });
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Did not restore .env: .env is ignored by git."));
    let _ = std::fs::remove_dir_all(&dir);
}

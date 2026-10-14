//! Headless tests for the file actions: the restore confirmation and its refusals, the effect
//! and result handling, and opening or showing working-tree files.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, FileEntry, LoadOptions, RestoredFrom};

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
    Msg::RestoreFile { oid, path: path.into(), deleted, confirmed: false }
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
    assert!(matches!(&**then, Msg::RestoreFile { oid: o, path, deleted: false, confirmed: true } if *o == oid && path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_warns_about_local_changes() {
    let (mut st, dir) = loaded("files-warn");
    let oid = head(&dir);
    run(&mut st, ask(oid, "a.txt", false));
    assert!(confirm_body(&st).contains("overwritten"), "{}", confirm_body(&st));
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
        matches!(&fx[..], [Effect::RestoreFile { workdir, oid: o, path }] if *workdir == dir && *o == oid && path == "a.txt"),
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

    let fx = run(&mut st, Msg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Commit) });
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok && toast.text.contains("a.txt") && toast.text.contains(&short(oid)), "{}", toast.text);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    assert!(st.detail.as_ref().unwrap().in_workdir("a.txt"), "Open and Show become available");

    run(&mut st, Msg::FileRestored { path: "a.txt".into(), oid, result: Ok(RestoredFrom::Parent(oid)) });
    assert!(st.toasts.last().unwrap().text.contains("before"));

    let fx = run(&mut st, Msg::FileRestored { path: "a.txt".into(), oid, result: Err("boom".into()) });
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not restore a.txt: boom"));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scoped_error_for_inactive_tab_is_logged() {
    let (mut st, dir) = loaded("files-scoped");
    let msg = Msg::FileRestored { path: "b.txt".into(), oid: head(&dir), result: Err("in the way".into()) };
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
    drive(&mut st, Msg::Open(dir.clone()));
    assert!(st.snap.as_ref().unwrap().unstaged.is_empty());
    drive(&mut st, Msg::RestoreFile { oid: first, path: "a.txt".into(), deleted: false, confirmed: true });
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"), "shows as an unstaged change");
    assert!(st.snap.as_ref().unwrap().staged.is_empty(), "nothing staged");
    assert!(st.toasts.last().is_some_and(|t| t.ok));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_and_show_build_absolute_paths() {
    let mut st = state();
    assert!(run(&mut st, Msg::OpenWorkingFile("src/a.txt".into())).is_empty(), "no repository open");
    assert!(run(&mut st, Msg::ShowWorkingFile("src/a.txt".into())).is_empty());

    let (mut st, dir) = loaded("files-open");
    let fx = run(&mut st, Msg::OpenWorkingFile("src/a.txt".into()));
    assert!(matches!(&fx[..], [Effect::OpenPath(p)] if *p == dir.join("src/a.txt")), "{fx:?}");
    let fx = run(&mut st, Msg::ShowWorkingFile("src/a.txt".into()));
    assert!(matches!(&fx[..], [Effect::RevealPath(p)] if *p == dir.join("src/a.txt")), "{fx:?}");
    assert_eq!(fx[0].scope(), None, "not tied to a tab");
    let _ = std::fs::remove_dir_all(&dir);
}

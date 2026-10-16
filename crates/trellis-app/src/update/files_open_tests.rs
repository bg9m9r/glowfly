//! Headless tests for opening a working-tree file that the system would run: the question
//! first (worded by the repository's trust), a refusal for a repository the user doesn't trust,
//! and the runner's own finding (an executable bit) coming back as the same question.

use super::tests::{scratch_repo, state};
use super::update;
use super::work_files_tests::last_log;
use crate::effect::Effect;
use crate::msg::{FilesMsg, Msg};
use crate::state::trust::record;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::opens::Os;
use trellis_core::repo::{self, LoadOptions};
use trellis_core::settings::RepoTrust;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn loaded(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

/// A name this system runs when it is opened.
fn program() -> (&'static str, &'static str) {
    match Os::current() {
        Os::Windows => ("tools/run.bat", "a script"),
        Os::MacOs => ("tools/run.command", "a script"),
        Os::Linux => ("tools/run.desktop", "a launcher"),
    }
}

fn trust(st: &mut AppState, t: RepoTrust) {
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    record(&mut st.settings, &real, t);
}

fn confirm(st: &AppState) -> (&str, &str, &str, &Msg) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, then }) => (title, body, button, then),
        _ => panic!("expected the confirmation"),
    }
}

#[test]
fn opening_a_program_asks_first_and_runs_it_once_agreed() {
    let (mut st, dir) = loaded("open-program");
    let (path, kind) = program();
    assert!(run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile(path.into()))).is_empty(), "nothing opened yet");
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Run this file?", "Run"));
    assert!(body.starts_with(&format!("{path} is {kind}. Opening it with its default app runs it")), "{body}");
    assert!(body.contains("You haven't said whether you trust this repository."), "{body}");
    assert!(body.ends_with("To read or edit it instead, use Open in external editor."), "{body}");
    assert!(matches!(then, Msg::Files(FilesMsg::OpenConfirmed(p)) if p == path));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(matches!(&fx[..], [Effect::OpenPath { workdir, path: p, confirmed: true }] if *workdir == dir && p == path), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trusted_repository_still_asks_without_the_trust_note() {
    let (mut st, dir) = loaded("open-program-trusted");
    trust(&mut st, RepoTrust::Trusted);
    run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile(program().0.into())));
    let (_, body, ..) = confirm(&st);
    assert!(!body.contains("trust this repository"), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untrusted_repository_refuses_to_run_it() {
    let (mut st, dir) = loaded("open-program-untrusted");
    trust(&mut st, RepoTrust::NotTrusted);
    let (path, kind) = program();
    assert!(run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile(path.into()))).is_empty());
    assert!(st.modal.is_none());
    let last = last_log(&st);
    assert_eq!(
        last,
        format!(
            "Did not open {path}: it is {kind}, and opening it would run it. You chose not to trust this repository. To read it, use Open in external editor."
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ordinary_files_open_unasked_and_the_runner_can_still_ask() {
    let (mut st, dir) = loaded("open-plain");
    let fx = run(&mut st, Msg::Files(FilesMsg::OpenWorkingFile("notes.txt".into())));
    assert!(matches!(&fx[..], [Effect::OpenPath { path, confirmed: false, .. }] if path == "notes.txt"), "{fx:?}");
    assert!(st.modal.is_none());
    // The runner found an executable bit: the same question.
    let found = FilesMsg::OpenNeedsConfirm { workdir: dir.clone(), path: "bin/run".into(), kind: "an executable file" };
    assert!(run(&mut st, Msg::Files(found)).is_empty());
    let (_, body, _, then) = confirm(&st);
    assert!(body.starts_with("bin/run is an executable file."), "{body}");
    assert!(matches!(then, Msg::Files(FilesMsg::OpenConfirmed(p)) if p == "bin/run"));
    // For a repository that is no longer the active one, it is only logged.
    st.modal = None;
    let elsewhere = FilesMsg::OpenNeedsConfirm { workdir: dir.join("other"), path: "bin/run".into(), kind: "an executable file" };
    run(&mut st, Msg::Files(elsewhere));
    assert!(st.modal.is_none());
    assert!(last_log(&st).starts_with("Did not open bin/run: it is an executable file"), "{}", last_log(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

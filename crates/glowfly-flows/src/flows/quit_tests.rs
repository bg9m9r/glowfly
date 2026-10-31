//! Asking before quitting: File > Quit, the Quit key, the title bar's Close and the OS closing
//! the window go through one check that asks only while a git job (not a quiet auto-fetch) or a
//! fetch or pull across repositories runs, and never again once Quit went ahead.

use super::tests::state;
use super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::{AppMsg, Msg};
use crate::state::palette::Palette;
use crate::state::quit_confirm::quit_confirm;
use crate::state::repo_sync::{RepoSync, RowState, SyncKind, SyncRow, SyncScope};
use crate::state::{AppState, Modal, RunningJob, Tab};
use glowfly_core::repo_sync::Outcome;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn quit(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::App(AppMsg::Quit))
}

fn os_close(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::App(AppMsg::CloseRequested))
}

/// An absolute folder on every OS (never opened).
fn folder(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join("glowfly-quit-tests").join(name)
}

fn job(label: &str, workdir: Option<PathBuf>, quiet: bool) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir,
        quiet,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    }
}

/// A state with a tab at `api-server` labelled "api" and a Push running there.
pub(crate) fn pushing() -> AppState {
    let mut st = state();
    let mut tab = Tab::unloaded(folder("api-server"));
    tab.label = Some("api".into());
    st.tabs.push(tab);
    st.job = Some(job("Push", Some(folder("api-server")), false));
    st
}

fn question(st: &AppState) -> (&str, &str, &str, &Msg) {
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, then, .. }) => (title, body, button, then),
        _ => panic!("no quit question"),
    }
}

fn closes(fx: &[Effect]) -> bool {
    matches!(fx, [Effect::Ui(UiEffect::CloseWindow)])
}

fn cancels(fx: &[Effect]) -> bool {
    matches!(fx, [Effect::Ui(UiEffect::CancelClose)])
}

#[test]
fn quit_without_work_closes_and_marks_quitting() {
    let mut st = state();
    let fx = quit(&mut st);
    assert!(closes(&fx), "{fx:?}");
    assert!(st.view.quitting);
    assert!(st.modal.is_none() && st.toasts.is_empty());
}

#[test]
fn quit_while_a_job_runs_asks_and_names_it() {
    let mut st = pushing();
    let fx = quit(&mut st);
    assert!(fx.is_empty(), "{fx:?}");
    let (title, body, button, then) = question(&st);
    assert_eq!(title, "Quit Glowfly?");
    assert!(body.starts_with("Push is still running in api.\n\n"), "{body}");
    assert!(body.ends_with("Quitting now can cut it off partway, and Glowfly won't see how it ends."), "{body}");
    assert_eq!(button, "Quit anyway");
    assert!(matches!(then, Msg::App(AppMsg::QuitConfirmed)));
    assert!(!st.view.quitting);
}

#[test]
fn quit_names_a_job_without_a_repository() {
    let mut st = state();
    st.job = Some(job("Push", None, false));
    quit(&mut st);
    assert!(question(&st).1.starts_with("Push is still running.\n\n"), "{}", question(&st).1);
    // A repository no tab shows: its folder's name.
    let mut st = state();
    st.job = Some(job("Fetch", Some(folder("docs")), false));
    quit(&mut st);
    assert!(question(&st).1.starts_with("Fetch is still running in docs."), "{}", question(&st).1);
}

#[test]
fn quit_anyway_closes() {
    let mut st = pushing();
    quit(&mut st);
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(closes(&fx), "{fx:?}");
    assert!(st.view.quitting);
    assert!(st.modal.is_none());
    assert!(st.job.is_some(), "the job is left alone");
}

#[test]
fn cancel_keeps_the_window_and_the_job() {
    let mut st = pushing();
    quit(&mut st);
    let fx = run(&mut st, Msg::CloseModal);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.modal.is_none());
    assert!(!st.view.quitting);
    assert!(st.job.is_some());
}

#[test]
fn a_quiet_job_never_asks() {
    let mut st = state();
    st.job = Some(job("Fetch", Some(folder("api-server")), true));
    let fx = quit(&mut st);
    assert!(closes(&fx), "{fx:?}");
    assert!(st.modal.is_none());
}

#[test]
fn repo_sync_running_asks() {
    let mut st = state();
    let row = |name: &str, state: RowState| SyncRow { path: folder(name), name: name.into(), state };
    st.repo_sync = Some(RepoSync {
        generation: 1,
        kind: SyncKind::Pull,
        scope: SyncScope::Listed,
        rows: vec![row("api", RowState::Running), row("docs", RowState::Done(Outcome::UpToDate))],
    });
    assert!(quit(&mut st).is_empty());
    assert!(question(&st).1.starts_with("Pull across repositories is still running (1 of 2 done).\n\n"), "{}", question(&st).1);
    // Finished: nothing to ask about.
    let mut st = state();
    st.repo_sync = Some(RepoSync {
        generation: 1,
        kind: SyncKind::Fetch,
        scope: SyncScope::Favorites,
        rows: vec![row("docs", RowState::Done(Outcome::UpToDate))],
    });
    assert!(closes(&quit(&mut st)));
}

#[test]
fn os_close_without_work_lets_it_close() {
    let mut st = state();
    let fx = os_close(&mut st);
    assert!(fx.is_empty(), "nothing cancels it: {fx:?}");
    assert!(st.view.quitting);
}

#[test]
fn os_close_while_work_runs_cancels_and_asks() {
    let mut st = pushing();
    let fx = os_close(&mut st);
    assert!(cancels(&fx), "{fx:?}");
    assert_eq!(question(&st).0, "Quit Glowfly?");
}

#[test]
fn os_close_after_quit_anyway_passes() {
    let mut st = pushing();
    os_close(&mut st);
    assert!(closes(&run(&mut st, Msg::App(AppMsg::QuitConfirmed))));
    let fx = os_close(&mut st);
    assert!(fx.is_empty(), "the close Quit anyway started isn't asked about: {fx:?}");
    assert!(closes(&quit(&mut st)), "Quit again just closes");
}

#[test]
fn asking_again_keeps_one_question() {
    let mut st = pushing();
    quit(&mut st);
    let before = question(&st).1.to_owned();
    // The job moves on meanwhile: the open question isn't rebuilt.
    st.job = Some(job("Fetch", None, false));
    assert!(quit(&mut st).is_empty());
    assert_eq!(question(&st).1, before);
    assert!(cancels(&os_close(&mut st)));
    assert_eq!(question(&st).1, before);
}

#[test]
fn the_question_replaces_another_dialog_and_closes_the_palette() {
    let mut st = pushing();
    st.modal = Some(Modal::About);
    st.palette = Some(Palette::default());
    quit(&mut st);
    assert_eq!(question(&st).0, "Quit Glowfly?");
    assert!(st.palette.is_none());
}

#[test]
fn quit_messages_have_nothing_for_an_inactive_tab() {
    for m in [AppMsg::Quit, AppMsg::CloseRequested, AppMsg::QuitConfirmed] {
        assert!(m.clone().on_inactive().is_none(), "{m:?}");
    }
}

#[test]
fn the_question_is_translated() {
    let mut st = pushing();
    st.settings.language = "de".into();
    let Some(Modal::ConfirmDanger { title, body, button, .. }) = quit_confirm(&st, &[]) else { panic!("no question") };
    assert_eq!(title, "Glowfly beenden?");
    assert!(body.starts_with("Push läuft noch in api."), "{body}");
    assert_eq!(button, "Trotzdem beenden");
    assert!(quit_confirm(&state(), &[]).is_none(), "nothing runs");
}

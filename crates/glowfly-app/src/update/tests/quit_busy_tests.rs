//! Headless tests for the closes that end terminals or cut off work (#187): quitting, closing a
//! repository tab and Install and restart ask the runner what their running shells are doing
//! and ask the user only while one runs a program; Quit anyway stops a fetch before closing;
//! the quit question closes the window by itself once what it named has ended; a sparse clone
//! and an update's download ask too. No shell runs here: the runner's answers are sent by hand.

use super::quit_tests::pushing;
use super::terminal_tests::{open_with_panel, run as term, workdir};
use super::update;
use super::updates_restart_tests::msi_downloaded;
use crate::effect::partial::StopKey;
use crate::effect::{Effect, UiEffect, UpdateEffect};
use crate::msg::{AppMsg, CloneMsg, JobMsg, Msg, RepoMsg, TabsMsg, UpdateMsg};
use crate::state::clone::CloneRequest;
use crate::state::job_stop::JobStop;
use crate::state::menu_bar::tab_name;
use crate::state::terminal_names::name_of;
use crate::state::{AppState, Modal, RunningJob, Tab};
use crate::terminal::{BusyCheck, TerminalEffect as E, TerminalMsg as T};
use glowfly_core::clone_ops::{CloneOptions, Sparse};
use glowfly_core::terminal::ShellActivity;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A repository open with its terminal panel's first shell running; returns its session id.
fn shell_running(name: &str) -> (AppState, PathBuf, u64) {
    let (mut st, dir, id) = open_with_panel(name);
    term(&mut st, T::Started { id, program: "pwsh".into() });
    (st, dir, id)
}

/// The `CheckBusy` among `fx`, if any.
fn check(fx: &[Effect]) -> Option<(Vec<u64>, BusyCheck)> {
    fx.iter().find_map(|e| match e {
        Effect::Terminal(E::CheckBusy { ids, then }) => Some((ids.clone(), *then)),
        _ => None,
    })
}

fn answer(st: &mut AppState, id: u64, activity: ShellActivity, then: BusyCheck) -> Vec<Effect> {
    term(st, T::BusyChecked { found: vec![(id, activity)], then })
}

fn cargo() -> ShellActivity {
    ShellActivity::Running { program: Some("cargo".into()) }
}

fn closes(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::CloseWindow)))
}

fn dialog(st: &AppState) -> (&str, &str, &Msg) {
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, then, .. }) => (title, body, then),
        _ => panic!("no confirmation"),
    }
}

/// "<terminal> in <tab> is running cargo." for session `id` of the active tab.
fn cargo_line(st: &AppState, id: u64) -> String {
    format!("{} in {} is running cargo.", name_of(st, id).unwrap(), tab_name(st, st.active_tab))
}

const STOPS_ONE: &str = "Whatever runs in that terminal stops with it.";

fn job(label: &str, workdir: Option<PathBuf>) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn quit_with_a_busy_terminal_asks_and_names_the_tab_and_program() {
    let (mut st, dir, id) = shell_running("quit-busy-term");
    let fx = run(&mut st, Msg::App(AppMsg::Quit));
    assert_eq!(check(&fx), Some((vec![id], BusyCheck::Quit)), "{fx:?}");
    assert!(!closes(&fx) && st.modal.is_none() && !st.view.quitting, "nothing closes before the answer");

    let fx = answer(&mut st, id, cargo(), BusyCheck::Quit);
    assert!(!closes(&fx), "{fx:?}");
    let (title, body, then) = dialog(&st);
    assert_eq!(title, "Quit Glowfly?");
    assert_eq!(body, format!("{}\n\n{STOPS_ONE}", cargo_line(&st, id)));
    assert!(matches!(then, Msg::App(AppMsg::QuitConfirmed)));
    // Cancel keeps everything.
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && !st.view.quitting && st.view.quit_ask.is_none());
    assert_eq!(st.terminal.sessions.len(), 1);
    // Asked again and accepted: the window closes.
    run(&mut st, Msg::App(AppMsg::Quit));
    answer(&mut st, id, ShellActivity::Running { program: None }, BusyCheck::Quit);
    assert!(dialog(&st).1.contains("is running a program."), "{}", dialog(&st).1);
    assert!(closes(&run(&mut st, Msg::ConfirmAccepted)));
    assert!(st.view.quitting);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_with_an_idle_or_unknown_terminal_closes() {
    for activity in [ShellActivity::Idle, ShellActivity::Unknown] {
        let (mut st, dir, id) = shell_running("quit-idle-term");
        run(&mut st, Msg::App(AppMsg::Quit));
        let fx = answer(&mut st, id, activity, BusyCheck::Quit);
        assert!(closes(&fx), "{fx:?}");
        assert!(st.view.quitting && st.modal.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_os_close_waits_for_the_answer() {
    let (mut st, dir, id) = shell_running("quit-os-term");
    let fx = run(&mut st, Msg::App(AppMsg::CloseRequested));
    assert!(fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::CancelClose))), "{fx:?}");
    assert!(check(&fx).is_some());
    let fx = answer(&mut st, id, ShellActivity::Idle, BusyCheck::Quit);
    assert!(closes(&fx), "the answer closes the window itself: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shell_still_starting_is_not_asked_about() {
    let (mut st, dir, _) = open_with_panel("quit-starting-term");
    let fx = run(&mut st, Msg::App(AppMsg::Quit));
    assert!(check(&fx).is_none() && closes(&fx), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_repository_tab_with_a_busy_terminal_asks() {
    let (mut st, dir, id) = shell_running("close-tab-busy-term");
    let tab = st.tabs[0].id;
    let fx = run(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    assert_eq!(check(&fx), Some((vec![id], BusyCheck::CloseTabs(crate::msg::CloseRequest::One(tab)))), "{fx:?}");
    assert_eq!(st.tabs[0].path.as_deref(), Some(dir.as_path()), "the tab stays until the answer");

    answer(&mut st, id, cargo(), BusyCheck::CloseTabs(crate::msg::CloseRequest::One(tab)));
    let (title, body, _) = dialog(&st);
    assert_eq!(title, "Close tab?");
    assert_eq!(body, format!("{}\n\n{STOPS_ONE}", cargo_line(&st, id)));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(fx.iter().any(|e| matches!(e, Effect::Terminal(E::Kill(k)) if *k == id)), "{fx:?}");
    assert!(st.terminal.sessions.is_empty());
    assert!(st.tabs[0].path.is_none(), "the last tab becomes a new tab");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_repository_tab_with_an_idle_terminal_just_closes() {
    let (mut st, dir, id) = shell_running("close-tab-idle-term");
    let req = crate::msg::CloseRequest::One(st.tabs[0].id);
    run(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    let fx = answer(&mut st, id, ShellActivity::Idle, BusyCheck::CloseTabs(req));
    assert!(st.modal.is_none());
    assert!(fx.iter().any(|e| matches!(e, Effect::Terminal(E::Kill(k)) if *k == id)), "{fx:?}");
    assert!(st.terminal.sessions.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_another_tab_of_the_same_repository_never_asks() {
    let (mut st, dir, _) = shell_running("close-tab-shared-term");
    let mut twin = Tab::unloaded(dir.clone());
    twin.label = Some("twin".into());
    st.tabs.push(twin);
    let fx = run(&mut st, Msg::Tabs(TabsMsg::CloseTab(1)));
    assert!(check(&fx).is_none(), "the shells stay with the other tab: {fx:?}");
    assert_eq!(st.tabs.len(), 1);
    assert_eq!(st.terminal.sessions.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

fn with_update_and_shell(name: &str) -> (AppState, Instant, PathBuf, u64) {
    let (mut st, t) = msi_downloaded();
    let dir = super::tests::scratch_repo(name);
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    term(&mut st, T::Toggle);
    let id = st.terminal.sessions[0].id;
    term(&mut st, T::Started { id, program: "pwsh".into() });
    assert_eq!(workdir(&st), st.terminal.sessions[0].workdir);
    (st, t, dir, id)
}

fn restarts(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::Restart(_))))
}

#[test]
fn install_and_restart_with_a_busy_terminal_asks() {
    let (mut st, t, dir, id) = with_update_and_shell("restart-busy-term");
    let fx = update(&mut st, Msg::Updates(UpdateMsg::InstallAndRestart), t);
    assert_eq!(check(&fx), Some((vec![id], BusyCheck::Restart)), "{fx:?}");
    assert!(!restarts(&fx));
    let fx = answer(&mut st, id, cargo(), BusyCheck::Restart);
    assert!(!restarts(&fx));
    let (title, body, then) = dialog(&st);
    assert_eq!(title, "Restart to update?");
    assert_eq!(body, format!("Restarting closes every tab.\n{}\n\n{STOPS_ONE}", cargo_line(&st, id)));
    assert!(matches!(then, Msg::Updates(UpdateMsg::InstallAndRestartConfirmed)));
    assert!(restarts(&run(&mut st, Msg::ConfirmAccepted)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn install_and_restart_with_an_idle_terminal_restarts() {
    let (mut st, t, dir, id) = with_update_and_shell("restart-idle-term");
    update(&mut st, Msg::Updates(UpdateMsg::InstallAndRestart), t);
    let fx = answer(&mut st, id, ShellActivity::Idle, BusyCheck::Restart);
    assert!(restarts(&fx), "{fx:?}");
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A state with a Fetch running in a tab, stoppable like a real one.
fn fetching() -> AppState {
    let mut st = pushing();
    st.job = Some(RunningJob { fetches: true, ..job("Fetch", st.tabs.last().and_then(|t| t.path.clone())) });
    st.job_stop = Some(JobStop::default());
    st
}

#[test]
fn quit_anyway_stops_a_fetch_and_closes_once_it_has_ended() {
    let mut st = fetching();
    run(&mut st, Msg::App(AppMsg::Quit));
    assert!(dialog(&st).1.starts_with("Fetch is still running in api."), "{}", dialog(&st).1);
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(fx.iter().any(|e| matches!(e, Effect::StopGit(StopKey::Job))), "the stop is sent: {fx:?}");
    assert!(!closes(&fx), "the window waits for the stopped fetch: {fx:?}");
    assert!(st.view.quit_after_stop && !st.view.quitting);
    let output = "Cancelled.\nRemoved 1 file the stopped command left behind.".into();
    let fx = run(&mut st, Msg::Job(JobMsg::Stopped { output, undo: None }));
    assert!(closes(&fx), "{fx:?}");
    assert!(st.view.quitting && !st.view.quit_after_stop);
}

#[test]
fn quitting_again_while_a_stopped_fetch_ends_closes_at_once() {
    let mut st = fetching();
    run(&mut st, Msg::App(AppMsg::Quit));
    run(&mut st, Msg::ConfirmAccepted);
    assert!(closes(&run(&mut st, Msg::App(AppMsg::Quit))));
    assert!(st.view.quitting);
}

#[test]
fn quit_anyway_leaves_a_job_that_cant_be_stopped() {
    let mut st = pushing();
    run(&mut st, Msg::App(AppMsg::Quit));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(closes(&fx) && !fx.iter().any(|e| matches!(e, Effect::StopGit(_))), "{fx:?}");
}

#[test]
fn the_window_closes_by_itself_once_the_job_ends_under_the_question() {
    let mut st = pushing();
    run(&mut st, Msg::App(AppMsg::Quit));
    assert!(crate::state::quit_confirm::asking(&st));
    let fx = run(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(closes(&fx), "{fx:?}");
    assert!(st.modal.is_none() && st.view.quitting);
}

#[test]
fn the_question_drops_what_ended_and_closes_once_nothing_is_left() {
    let (mut st, dir, id) = shell_running("quit-job-and-term");
    st.job = Some(job("Push", Some(workdir(&st))));
    run(&mut st, Msg::App(AppMsg::Quit));
    answer(&mut st, id, cargo(), BusyCheck::Quit);
    let tab = tab_name(&st, 0).to_owned();
    let line = cargo_line(&st, id);
    let risk = "Quitting now can cut it off partway, and Glowfly won't see how it ends.";
    assert_eq!(dialog(&st).1, format!("Push is still running in {tab}.\n{line}\n\n{risk} {STOPS_ONE}"));
    // The push ends: the question names only the terminal.
    let fx = run(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(!closes(&fx), "{fx:?}");
    assert_eq!(dialog(&st).1, format!("{line}\n\n{STOPS_ONE}"));
    // Its shell ends: nothing is left, the window closes.
    let fx = term(&mut st, T::Exited { id, code: Some(0) });
    assert!(closes(&fx), "{fx:?}");
    assert!(st.view.quitting);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sparse_clone_asks_before_quitting() {
    let mut st = super::tests::state();
    let parent = glowfly_core::paths::temp_dir().join("glowfly-quit-tests").join("work");
    let sparse = Some(Sparse { folders: vec!["docs".into()], partial: false });
    let req = CloneRequest {
        url: "https://example.com/team/app.git".into(),
        parent,
        name: "app".into(),
        options: CloneOptions { shallow: None, sparse },
    };
    run(&mut st, Msg::Clone(CloneMsg::Start(req)));
    assert!(st.job.is_some(), "a sparse clone runs as a job");
    let fx = run(&mut st, Msg::App(AppMsg::Quit));
    assert!(!closes(&fx), "{fx:?}");
    assert!(dialog(&st).1.starts_with("Clone is still running.\n\n"), "{}", dialog(&st).1);
}

#[test]
fn an_update_download_asks_before_quitting() {
    let (mut st, t) = super::updates_tests::offered();
    super::updates_tests::send(&mut st, UpdateMsg::Download, t);
    let fx = run(&mut st, Msg::App(AppMsg::Quit));
    assert!(!closes(&fx), "{fx:?}");
    assert!(dialog(&st).1.starts_with("Glowfly 0.3.0 is still downloading.\n\n"), "{}", dialog(&st).1);
}

#[test]
fn the_terminal_lines_are_translated() {
    let (mut st, dir, id) = shell_running("quit-busy-term-de");
    st.settings.language = "de".into();
    run(&mut st, Msg::App(AppMsg::Quit));
    answer(&mut st, id, cargo(), BusyCheck::Quit);
    let body = dialog(&st).1;
    assert!(body.ends_with("führt cargo aus.\n\nWas in diesem Terminal läuft, wird mit ihm beendet."), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

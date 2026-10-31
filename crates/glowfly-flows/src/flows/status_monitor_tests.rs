//! Repository > Hooks' file system monitor row: Turn off starts its job through the job gate,
//! and either job's end leaves the page showing what is now set.

use crate::effect::{Effect, MonitorOff};
use crate::flows::status_tip_tests::{opened, speedups};
use crate::msg::{LoadMsg, Msg};
use crate::state::prefs::{GitConfigValues, PrefsState};
use crate::state::{AppState, PrefPage, prefs_save};
use crate::update::update;
use glowfly_core::repo::MonitorOutcome;
use std::time::Instant;

fn on_hooks_page(st: &mut AppState, on: bool) {
    let mut p = PrefsState::new(PrefPage::RepoHooks);
    let values = GitConfigValues { fsmonitor: on, ..GitConfigValues::default() };
    p.git_saved = Some(values.clone());
    p.git = Some(values);
    st.prefs = Some(p);
}

fn shown(st: &AppState) -> (bool, bool) {
    let p = st.prefs.as_ref().unwrap();
    (p.git.as_ref().unwrap().fsmonitor, p.git_saved.as_ref().unwrap().fsmonitor)
}

#[test]
fn turn_off_sends_the_job() {
    let (mut st, dir) = opened("monitor-off-job");
    let workdir = st.workdir().unwrap();
    let fx = update(&mut st, Msg::Load(LoadMsg::TurnOffMonitor), Instant::now());
    assert!(matches!(&fx[..], [Effect::TurnOffMonitor(MonitorOff { cwd })] if cwd == &workdir), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Turn off the file system monitor"));
    // One job at a time.
    let again = update(&mut st, Msg::Load(LoadMsg::TurnOffMonitor), Instant::now());
    assert!(!again.iter().any(|e| matches!(e, Effect::TurnOffMonitor(_))), "{again:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_repository_sends_nothing() {
    let mut st = crate::flows::state();
    let fx = update(&mut st, Msg::Load(LoadMsg::TurnOffMonitor), Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
}

#[test]
fn the_page_follows_the_job() {
    let (mut st, dir) = opened("monitor-off-page");
    on_hooks_page(&mut st, true);
    update(&mut st, Msg::Load(LoadMsg::TurnOffMonitor), Instant::now());
    let output = "$ git config --unset-all core.fsmonitor\n$ git fsmonitor--daemon stop".to_owned();
    update(&mut st, Msg::Load(LoadMsg::MonitorTurnedOff { output, ok: true }), Instant::now());
    assert!(st.job.is_none(), "the job ended");
    assert_eq!(shown(&st), (false, false));
    let p = st.prefs.as_ref().unwrap();
    assert!(!prefs_save::unsaved(p, &st.settings, st.snap.as_ref()), "not an edit");
    assert!(st.toasts.last().is_some_and(|t| t.text.contains("off")), "{:?}", st.toasts.last().map(|t| &t.text));

    // A failed Turn off leaves it shown as on.
    on_hooks_page(&mut st, true);
    update(&mut st, Msg::Load(LoadMsg::TurnOffMonitor), Instant::now());
    update(&mut st, Msg::Load(LoadMsg::MonitorTurnedOff { output: "fatal: boom".into(), ok: false }), Instant::now());
    assert_eq!(shown(&st), (true, true));

    // Turn on from the page: shown as on once the monitor watches.
    on_hooks_page(&mut st, false);
    update(&mut st, Msg::Load(LoadMsg::SpeedUpStatus { speedups: speedups(true, false), cache: false, monitor: true }), Instant::now());
    let watching = MonitorOutcome::Watching { started: true };
    update(&mut st, Msg::Load(LoadMsg::StatusSpedUp { output: String::new(), monitor: watching, cache: None }), Instant::now());
    assert_eq!(shown(&st), (true, true));
    let _ = std::fs::remove_dir_all(&dir);
}

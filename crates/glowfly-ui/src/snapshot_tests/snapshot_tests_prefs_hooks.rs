//! Repository > Hooks' file system monitor row: Turn off while the repository's core.fsmonitor
//! is on, Turn on only where this OS and git have the daemon, and neither while a job runs.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{LoadMsg, Msg};
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::GitInfo;
use glowfly_core::opens::Os;
use glowfly_core::settings::ThemeChoice;

/// The Hooks page of a repository whose core.fsmonitor is `on`, with a git 2.45 that has the
/// daemon.
fn hooks_page(name: &str, on: bool) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let mut prefs = PrefsState::new(PrefPage::RepoHooks);
    let git = GitConfigValues { fsmonitor: on, ..Default::default() };
    prefs.git_saved = Some(git.clone());
    prefs.git = Some(git);
    st.prefs = Some(prefs);
    st.git.info = Some(GitInfo { path: Some("git".into()), version: Some((2, 45, 1)), chosen: false });
    st.git.monitor_daemon = Some(true);
    (st, dir)
}

#[test]
fn prefs_hooks_monitor_on() {
    let (st, dir) = hooks_page("prefs-hooks-monitor-on", true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_hooks_monitor_on");
    h.get_by_label("Turn off").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::TurnOffMonitor))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn turn_on_is_offered_only_where_git_has_the_daemon() {
    let offered = Os::current() != Os::Linux;
    for daemon in [Some(true), Some(false)] {
        let (mut st, dir) = hooks_page("prefs-hooks-monitor-off", false);
        st.git.monitor_daemon = daemon;
        let mut h = harness(st, true);
        h.run();
        let shown = offered && daemon == Some(true);
        assert_eq!(h.query_by_label("Turn on").is_some(), shown, "{daemon:?}");
        if shown {
            h.get_by_label("Turn on").click();
            h.run();
            let turn_on = |m: &Msg| matches!(m, Msg::Load(LoadMsg::SpeedUpStatus { cache: false, monitor: true, .. }));
            assert!(h.state().1.iter().any(turn_on));
        }
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn a_running_job_holds_both_buttons() {
    let (mut st, dir) = hooks_page("prefs-hooks-monitor-busy", true);
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: true,
    });
    let mut h = harness(st, true);
    h.run_steps(4);
    h.get_by_label("Turn off").click();
    h.run_steps(4);
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::TurnOffMonitor))));
    drop(h);
    remove_fixture(&dir);
}

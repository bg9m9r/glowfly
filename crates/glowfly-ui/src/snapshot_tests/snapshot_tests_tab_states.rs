//! The tabs' state slot (design pass D15): one tab per state, in Dark and Light (pixel goldens,
//! checked in the grey lens at sign-off), each tab's accessible name listing its states, and the
//! running job's dots standing still under reduce motion.

use crate::state::tabs::Parked;
use crate::state::tabs::glance::Glance;
use crate::state::{AppState, LogEntry, RunningJob, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::logs::RunInfo;
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};

fn place(name: &str) -> PathBuf {
    PathBuf::from("C:/work").join(name)
}

/// A parked tab for `name` whose last status read gave `g`.
fn parked(name: &str, g: Glance) -> Tab {
    let mut tab = Tab::unloaded(place(name));
    tab.parked = Some(Box::new(Parked { glance: Some(g), ..Parked::default() }));
    tab
}

fn running(dir: &Path) -> RunningJob {
    RunningJob {
        label: "Fetch all".into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: true,
    }
}

/// The fixture's tab, then a tab for each state the slot shows: running, failed, a merge in
/// progress, uncommitted changes and behind.
fn states(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.settings.reduce_motion = true;
    let upstream = Some("origin/main".to_owned());
    let mut failed = parked("web", Glance::default());
    let run = RunInfo { title: "Push".into(), summary: String::new(), took_ms: None, routine: false };
    failed.log.push(LogEntry { run: Some(run), ..LogEntry::new(false, "Push FAILED") });
    let first = st.tabs.remove(0);
    st.tabs = vec![
        first,
        parked("api-server", Glance { changed: 3, behind: 2, upstream: upstream.clone(), ..Glance::default() }),
        failed,
        parked("docs", Glance { op: Some(RepositoryState::Merge), conflicted: 1, ..Glance::default() }),
        parked("tools", Glance { changed: 3, ..Glance::default() }),
        parked("site", Glance { behind: 3, upstream, ..Glance::default() }),
    ];
    st.job = Some(running(&place("api-server")));
    (st, dir)
}

#[test]
fn tabs_states_dark() {
    let (st, dir) = states(ThemeChoice::Dark, "tabs-states-dark");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_states_dark");
    for name in [
        "api-server: Fetch all running, 3 changed files, 2 behind origin/main",
        "web: Push failed",
        "docs: Merge in progress, 1 conflicted file left",
        "tools: 3 changed files",
        "site: 3 behind origin/main",
    ] {
        assert!(h.query_by_label(name).is_some(), "{name}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tabs_states_light() {
    let (st, dir) = states(ThemeChoice::Light, "tabs-states-light");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("tabs_states_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_tab_without_states_is_named_by_its_name_alone() {
    let (mut st, dir) = states(ThemeChoice::Dark, "tabs-states-plain");
    st.job = None;
    st.tabs[5] = Tab::unloaded(place("site"));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("site").is_some(), "a tab not loaded yet");
    assert!(h.query_by_label("api-server: 3 changed files, 2 behind origin/main").is_some(), "no job any more");
    drop(h);
    remove_fixture(&dir);
}

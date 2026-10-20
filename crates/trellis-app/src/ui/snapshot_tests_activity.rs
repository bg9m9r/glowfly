//! The activity tab: runs with their result marks, summaries, durations and times, a routine
//! group of auto-fetches, a failed run opened with its cause, fix, Copy and output, and notes.

use super::snapshot_tests::{NOW, harness_sized, settings};
use crate::logs::LogsMsg;
use crate::msg::Msg;
use crate::state::logs::{EntryId, LogEntry, Persist};
use crate::state::{AppState, ToastAction};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::logs::RunInfo;
use trellis_core::settings::ThemeChoice;

const PULL_OUTPUT: &str = "$ git pull --ff\nerror: Your local changes to the following files would be overwritten by merge:\n\tf.txt\nhint: Commit or stash them first.\nPlease commit your changes or stash them before you merge.\nAborting";

/// A run that ended `minutes` ago.
fn run(ok: bool, title: &str, summary: &str, took_ms: Option<u64>, minutes: i64, output: &str) -> LogEntry {
    let status = if ok { "done" } else { "FAILED" };
    let routine = crate::state::activity_rows::is_routine_label(title);
    let info = RunInfo { title: title.into(), summary: summary.into(), took_ms, routine };
    LogEntry { at: NOW - minutes * 60, run: Some(info), ..LogEntry::new(ok, format!("{title} {status}\n{output}")) }
}

fn note(ok: bool, text: &str, minutes: i64) -> LogEntry {
    LogEntry { at: NOW - minutes * 60, ..LogEntry::new(ok, text) }
}

/// This session: a Push, five auto-fetches, a failed Pull (opened, with its fix) and two notes.
/// Returns the state and the ids of the Push, the first auto-fetch and the Pull.
fn session(theme: ThemeChoice) -> (AppState, [EntryId; 3]) {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.persist = Persist::On;
    let push = run(true, "Push", "Everything up-to-date", Some(1200), 42, "$ git push\nEverything up-to-date");
    let autos: Vec<LogEntry> =
        (0..5).map(|i| run(true, "Auto-fetch", "", Some(640 + 50 * i as u64), 30 - 5 * i, "$ git fetch --all")).collect();
    let pull = run(false, "Pull", "Your changes to 1 file would be overwritten.", Some(850), 3, PULL_OUTPUT);
    let ids = [push.id, autos[0].id, pull.id];
    st.log = vec![push];
    st.log.extend(autos);
    st.log.push(pull);
    st.log.push(note(false, "Couldn't reach github.com", 2));
    st.log.push(note(true, "Checked out main", 1));
    st.logs.open.insert(ids[2]);
    let fix = ToastAction { label: "Stash and pull".into(), msg: Box::new(Msg::git("Pull", &["pull", "--ff"])), workdir: None };
    st.logs.keep_fix(ids[2], fix);
    (st, ids)
}

/// A harness with the logs panel dragged as tall as it goes, so every row shows.
fn tall(mut st: AppState, dark: bool) -> Harness<'static, (AppState, Vec<Msg>)> {
    st.settings.panel_sizes.set(trellis_core::settings::SizedPanel::Activity, 470.0);
    let mut h = harness_sized(st, dark, eframe::egui::vec2(1400.0, 1000.0));
    h.run();
    h
}

#[test]
fn activity_runs_dark() {
    let (st, [push, _, pull]) = session(ThemeChoice::Dark);
    let mut h = tall(st, true);
    h.check_snapshot("activity_runs_dark");
    h.get_by_label("Stash and pull").click();
    h.run();
    h.get_by_label("Copy").click();
    h.run();
    h.get_by_label("Push, done, Everything up-to-date").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Fix(id)) if *id == pull)), "the fix button");
    let copied = msgs.iter().find_map(|m| match m {
        Msg::Copy(text) => Some(text.as_str()),
        _ => None,
    });
    assert_eq!(
        copied,
        Some(&*format!("Pull: Your changes to 1 file would be overwritten.\n{PULL_OUTPUT}")),
        "Copy has the title and output"
    );
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Fold(id)) if *id == push)), "a click folds the row");
}

#[test]
fn activity_runs_light() {
    let (st, _) = session(ThemeChoice::Light);
    let mut h = tall(st, false);
    h.check_snapshot("activity_runs_light");
}

#[test]
fn activity_routine_open() {
    let (mut st, [_, first, pull]) = session(ThemeChoice::Dark);
    st.logs.open.remove(&pull);
    st.logs.open.insert(first);
    let mut h = tall(st, true);
    h.check_snapshot("activity_routine_open");
    h.get_by_label("Auto-fetch, done, Ran 5 times").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Fold(id)) if *id == first)), "the group folds by its first run");
}

#[test]
fn an_empty_log_says_so() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.view.show_log = true;
    let h = tall(st, true);
    h.get_by_label("Nothing has happened in this tab yet.");
}

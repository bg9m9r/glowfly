//! The logs panel: earlier sessions' activity in dimmed groups, the error log's tab, and the
//! failure toast with Details.

use super::snapshot_tests::{NOW, harness, settings};
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::logs::{LogEntry, LogFileView, Persist};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::logs::{ActivityEntry, LogTail};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// 2026-01-29 09:30 UTC (three days before the harness's now) and a day later.
const FIRST: i64 = 1_769_679_000;
const SECOND: i64 = FIRST + 86_400;

fn earlier(session: i64, minutes: i64, ok: bool, text: &str) -> LogEntry {
    LogEntry::earlier(ActivityEntry { session, at: session + minutes * 60, ok, text: text.into(), repo: None, run: None })
}

#[test]
fn activity_log_earlier_sessions() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.persist = Persist::On;
    st.log = vec![
        earlier(FIRST, 1, true, "Fetched origin"),
        // A run line an older build wrote: shown as a run.
        earlier(FIRST, 2, true, "Fetch done\n$ git fetch origin\nFrom example.com:team/trellis"),
        earlier(FIRST, 4, false, "Push failed: the remote has commits you don't have"),
        earlier(SECOND, 2, true, "Checked out main"),
        LogEntry { at: NOW - 300, ..LogEntry::new(true, "Pulled origin/main") },
        LogEntry { at: NOW - 60, ..LogEntry::new(false, "Couldn't reach github.com") },
    ];
    // A panel dragged taller than its default, so every group shows.
    st.settings.panel_sizes.set(glowfly_core::settings::SizedPanel::Activity, 380.0);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("activity_log_earlier_sessions");
}

#[test]
fn activity_log_error_tab() {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.tab = LogKind::Errors;
    let lines = [
        "2026-10-16 09:30:01.120 +00:00 ERROR trellis-load-repo glowfly::runner::guard: effect panicked: index out of bounds",
        "2026-10-16 09:30:04.481 +00:00  WARN main glowfly::themes: theme file sand-dark.json: unknown color \"glow\"",
        "2026-10-16 09:31:12.007 +00:00  INFO trellis-git glowfly::runner: fetch finished",
        "2026-10-16 09:31:12.010 +00:00  INFO main glowfly::ui: window focused",
    ];
    let tail = LogTail { lines: lines.iter().map(|l| l.to_string()).collect(), truncated: true };
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Ok(tail)) });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("activity_log_error_tab");
    h.get_by_label("Performance").click();
    h.run();
    h.get_by_label("Open folder").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Show(LogKind::Performance)))));
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::OpenFolder))));
}

/// Preferences > General's Extended Logging and Logs rows, near the end of a tall page.
#[test]
fn preferences_general_logging() {
    use crate::state::{PrefPage, PrefsState};
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.extended_logging = true;
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = super::snapshot_tests::harness_sized(st, true, eframe::egui::vec2(1400.0, 1340.0));
    h.run();
    h.check_snapshot("prefs_general_logging");
    h.get_by_label("Add the git commands Trellis runs, with their timings, to the activity log").click();
    h.run();
    h.get_by_label("Open logs folder").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::SetExtended(false)))), "the checkbox sends a message");
    assert!(h.state().0.settings.extended_logging, "and doesn't change the setting itself");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::OpenFolder))));
}

/// A failure announced by a toast with Details instead of opening the panel: red, with a
/// button.
#[test]
fn toast_failure_details() {
    let (mut st, dir) = super::snapshot_tests::with_repo(ThemeChoice::Dark, "failure-details");
    crate::update::update(&mut st, Msg::Logged { ok: false, text: "Couldn't reach github.com".into() }, Instant::now());
    assert!(!st.view.show_log);
    let id = st.toasts.last().map(|t| t.id).unwrap();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("toast_failure_details");
    h.get_by_label("Details").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ToastAction(t) if *t == id)));
    drop(h);
    super::snapshot_tests::remove_fixture(&dir);
}

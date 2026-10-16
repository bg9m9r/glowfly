//! The logs panel: earlier sessions' activity in dimmed groups, and the error log's tab.

use super::snapshot_tests::{harness, settings};
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::logs::{LogEntry, LogFileView, Persist};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::logs::{ActivityEntry, LogTail};
use trellis_core::settings::ThemeChoice;

/// 2026-10-14 09:30 UTC and a day later.
const FIRST: i64 = 1_791_970_200;
const SECOND: i64 = FIRST + 86_400;

fn earlier(session: i64, minutes: i64, ok: bool, text: &str) -> LogEntry {
    LogEntry::earlier(ActivityEntry { session, at: session + minutes * 60, ok, text: text.into() })
}

#[test]
fn activity_log_earlier_sessions() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.persist = Persist::On;
    st.log = vec![
        earlier(FIRST, 1, true, "Fetched origin"),
        earlier(FIRST, 4, false, "Push failed: the remote has commits you don't have"),
        earlier(SECOND, 2, true, "Checked out main"),
        LogEntry::new(true, "Pulled origin/main"),
        LogEntry::new(false, "Couldn't reach github.com"),
    ];
    let mut h = harness(st, true);
    // A panel dragged taller than its default, so every group shows.
    let rect = eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 500.0), eframe::egui::vec2(1400.0, 290.0));
    h.ctx.data_mut(|d| d.insert_persisted(eframe::egui::Id::new("activity"), eframe::egui::panel::PanelState { rect }));
    h.run();
    h.check_snapshot("activity_log_earlier_sessions");
}

#[test]
fn activity_log_error_tab() {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.tab = LogKind::Errors;
    let lines = [
        "2026-10-16 09:30:01.120 +00:00 ERROR trellis-load-repo trellis::runner::guard: effect panicked: index out of bounds",
        "2026-10-16 09:30:04.481 +00:00  WARN main trellis::themes: theme file sand-dark.json: unknown color \"glow\"",
        "2026-10-16 09:31:12.007 +00:00  INFO trellis-git trellis::runner: fetch finished",
        "2026-10-16 09:31:12.010 +00:00  INFO main trellis::ui: window focused",
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
    let mut h = super::snapshot_tests::harness_sized(st, true, eframe::egui::vec2(1400.0, 1260.0));
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

//! Headless tests for the palette's log entries: offered with or without a repository, what
//! running each does, and how they are remembered.

use super::super::tests::state;
use super::super::update;
use crate::effect::Effect;
use crate::logs::{LogKind, LogsEffect, LogsMsg};
use crate::msg::{Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, entries, shown};
use std::time::Instant;

fn run_cmd(st: &mut AppState, cmd: C) -> Vec<Effect> {
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now())
}

fn labels(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).collect()
}

#[test]
fn log_entries_without_a_repository() {
    let mut st = state();
    let all = labels(&st);
    for label in
        ["View activity log", "View error log", "View performance log", "Open logs folder", "Export logs…", "Turn on extended logging"]
    {
        assert!(all.iter().any(|l| l == label), "{label} missing from {all:?}");
    }
    // After the external tools entries.
    let tools = all.iter().position(|l| l == "Configure external tools").unwrap();
    assert!(all.iter().position(|l| l == "View activity log").unwrap() > tools);
    st.settings.extended_logging = true;
    let all = labels(&st);
    assert!(all.iter().any(|l| l == "Turn off extended logging") && !all.iter().any(|l| l == "Turn on extended logging"));
    assert!(shown(&st, "crash").iter().any(|s| s.entry.label == "View error log"), "keywords match");
}

#[test]
fn running_log_entries() {
    let mut st = state();
    let fx = run_cmd(&mut st, C::Logs(LogsMsg::Show(LogKind::Performance)));
    assert!(fx.iter().any(|e| matches!(e, Effect::Logs(LogsEffect::ReadFile { kind: LogKind::Performance, .. }))), "{fx:?}");
    assert!(st.view.show_log);
    assert_eq!(st.logs.tab, LogKind::Performance);
    let fx = run_cmd(&mut st, C::Logs(LogsMsg::OpenFolder));
    assert!(fx.iter().any(|e| matches!(e, Effect::Logs(LogsEffect::OpenFolder))), "{fx:?}");
    let fx = run_cmd(&mut st, C::Logs(LogsMsg::Export));
    assert!(fx.iter().any(|e| matches!(e, Effect::Logs(LogsEffect::PickExport { .. }))), "{fx:?}");
    let fx = run_cmd(&mut st, C::Logs(LogsMsg::SetExtended(true)));
    assert!(fx.iter().any(|e| matches!(e, Effect::Logs(LogsEffect::SetExtended(true)))), "{fx:?}");
    assert!(st.settings.extended_logging);
}

#[test]
fn log_recall_keys() {
    let key = |m| C::Logs(m).recall_key();
    assert_eq!(key(LogsMsg::Show(LogKind::Activity)), Some("view-activity-log"));
    assert_eq!(key(LogsMsg::Show(LogKind::Errors)), Some("view-error-log"));
    assert_eq!(key(LogsMsg::Show(LogKind::Performance)), Some("view-performance-log"));
    assert_eq!(key(LogsMsg::OpenFolder), Some("open-logs-folder"));
    assert_eq!(key(LogsMsg::Export), Some("export-logs"));
    assert_eq!(key(LogsMsg::SetExtended(true)), key(LogsMsg::SetExtended(false)));
    assert_eq!(key(LogsMsg::SetExtended(true)), Some("extended-logging"));
    assert_eq!(key(LogsMsg::Start), None);

    let mut st = state();
    run_cmd(&mut st, C::Logs(LogsMsg::Show(LogKind::Errors)));
    assert_eq!(shown(&st, "")[0].entry.label, "View error log", "a recorded command ranks first");
    run_cmd(&mut st, C::Logs(LogsMsg::Export));
    assert_eq!(shown(&st, "")[0].entry.label, "Export logs…", "Export logs is remembered too");
    assert!(shown(&st, "bug report").iter().any(|s| s.entry.label == "Export logs…"), "keywords match");
}

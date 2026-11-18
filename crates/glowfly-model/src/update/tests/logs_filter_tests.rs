//! Headless tests for the activity panel's filters, folding of searched rows and list keys.

use super::super::tests::state;
use super::super::update;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::{Msg, TabsMsg};
use crate::state::AppState;
use crate::state::activity_filter::{Filter, row_ids};
use crate::state::activity_rows::is_routine_label;
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::logs::{EntryId, LogEntry};
use crate::state::tabs::Tab;
use glowfly_core::logs::RunInfo;
use std::time::Instant;

fn send(st: &mut AppState, m: LogsMsg) {
    update(st, Msg::Logs(m), Instant::now());
}

fn run(title: &str, ok: bool, output: &str) -> LogEntry {
    let status = if ok { "done" } else { "FAILED" };
    let info = RunInfo { title: title.into(), routine: is_routine_label(title), ..RunInfo::default() };
    LogEntry { run: Some(info), ..LogEntry::new(ok, format!("{title} {status}\n{output}")) }
}

fn ids(st: &AppState) -> Vec<EntryId> {
    st.log.iter().map(|e| e.id).collect()
}

fn key(st: &mut AppState, k: ListKey) {
    send(st, LogsMsg::RowKey(k));
}

fn step(down: bool) -> ListKey {
    ListKey::Move(ListMove::Step { down })
}

/// A panel showing the Activity tab over `entries`.
fn panel(entries: Vec<LogEntry>) -> AppState {
    let mut st = state();
    st.log = entries;
    st.view.show_log = true;
    st.logs.tab = LogKind::Activity;
    st
}

#[test]
fn set_failures_only_and_search_changed() {
    let mut st = state();
    send(&mut st, LogsMsg::SetFailuresOnly(true));
    assert!(st.logs.failures_only);
    send(&mut st, LogsMsg::SetFailuresOnly(false));
    assert!(!st.logs.failures_only);
    st.logs.search_closed.insert(EntryId::next());
    send(&mut st, LogsMsg::SearchChanged);
    assert!(st.logs.search_closed.is_empty());
}

#[test]
fn fold_closes_an_unfolded_match_and_reopens_it() {
    let mut st = panel(vec![run("Pull", true, "needle line")]);
    let id = st.log[0].id;
    st.view.logs_search = "needle".into();
    send(&mut st, LogsMsg::Fold(id));
    assert!(!st.logs.open.contains(&id));
    assert!(st.logs.search_closed.contains(&id));
    send(&mut st, LogsMsg::Fold(id));
    assert!(st.logs.open.contains(&id));
    assert!(!st.logs.search_closed.contains(&id));
    // Cleared, it stays open.
    st.view.logs_search.clear();
    send(&mut st, LogsMsg::SearchChanged);
    assert!(st.logs.open.contains(&id));

    // Without a search Fold is the plain toggle.
    send(&mut st, LogsMsg::Fold(id));
    assert!(!st.logs.open.contains(&id));
    assert!(st.logs.search_closed.is_empty());
    send(&mut st, LogsMsg::Fold(id));
    assert!(st.logs.open.contains(&id));
}

#[test]
fn arrows_move_the_cursor_over_the_shown_rows() {
    let routine = |ok| run("Auto-fetch", ok, "");
    let mut st = panel(vec![
        LogEntry::new(true, "one"),
        routine(true),
        routine(true),
        routine(true),
        run("Push", true, "x"),
        LogEntry::new(true, "two"),
    ]);
    let all = ids(&st);
    // Rows: one, group (first entry 1), Push, two.
    let stops = [all[0], all[1], all[4], all[5]];
    key(&mut st, step(true));
    assert_eq!(st.view.activity_focus, Some(stops[3]), "no cursor: the newest row");
    assert!(st.view.activity_focus_scroll);
    for want in [stops[2], stops[1], stops[0]] {
        st.view.activity_focus_scroll = false;
        key(&mut st, step(false));
        assert_eq!(st.view.activity_focus, Some(want));
        assert!(st.view.activity_focus_scroll);
    }
    key(&mut st, step(false));
    assert_eq!(st.view.activity_focus, Some(stops[0]), "stops at the oldest");
    key(&mut st, ListKey::Move(ListMove::Edge { end: true }));
    assert_eq!(st.view.activity_focus, Some(stops[3]));
    key(&mut st, step(true));
    assert_eq!(st.view.activity_focus, Some(stops[3]), "stops at the newest");
    key(&mut st, ListKey::Move(ListMove::Edge { end: false }));
    assert_eq!(st.view.activity_focus, Some(stops[0]));
    key(&mut st, ListKey::Move(ListMove::Page { down: true, rows: 2 }));
    assert_eq!(st.view.activity_focus, Some(stops[2]));
    key(&mut st, ListKey::Move(ListMove::Page { down: true, rows: 9 }));
    assert_eq!(st.view.activity_focus, Some(stops[3]));
    key(&mut st, ListKey::Move(ListMove::Page { down: false, rows: 3 }));
    assert_eq!(st.view.activity_focus, Some(stops[0]));

    // Home with no cursor lands on the oldest; Page Up on the newest.
    st.view.activity_focus = None;
    key(&mut st, ListKey::Move(ListMove::Edge { end: false }));
    assert_eq!(st.view.activity_focus, Some(stops[0]));
    st.view.activity_focus = None;
    key(&mut st, ListKey::Move(ListMove::Page { down: false, rows: 3 }));
    assert_eq!(st.view.activity_focus, Some(stops[3]));
}

#[test]
fn moves_follow_the_filter() {
    let mut st = panel(vec![run("Pull", false, "bad"), run("Push", true, ""), run("Fetch", true, ""), run("Merge", false, "bad")]);
    let all = ids(&st);
    send(&mut st, LogsMsg::SetFailuresOnly(true));
    st.view.activity_focus = Some(all[0]);
    key(&mut st, step(true));
    assert_eq!(st.view.activity_focus, Some(all[3]), "Down skips the hidden successes");
    // A cursor on a hidden row counts as none.
    st.view.activity_focus = Some(all[1]);
    key(&mut st, step(false));
    assert_eq!(st.view.activity_focus, Some(all[3]));
    st.view.activity_focus = Some(all[2]);
    key(&mut st, ListKey::Move(ListMove::Edge { end: false }));
    assert_eq!(st.view.activity_focus, Some(all[0]));
}

#[test]
fn enter_and_space_fold_the_cursor_row() {
    let mut st = panel(vec![
        LogEntry::new(true, "one line only"),
        run("Push", true, "out"),
        run("Auto-fetch", true, ""),
        run("Auto-fetch", true, ""),
    ]);
    let all = ids(&st);
    key(&mut st, ListKey::Toggle);
    assert!(st.logs.open.is_empty(), "no cursor, nothing");

    st.view.activity_focus = Some(all[0]);
    key(&mut st, ListKey::Toggle);
    key(&mut st, ListKey::Open);
    assert!(st.logs.open.is_empty(), "a one-line note does not fold");

    st.view.activity_focus = Some(all[1]);
    key(&mut st, ListKey::Toggle);
    assert!(st.logs.open.contains(&all[1]));
    key(&mut st, ListKey::Open);
    assert!(!st.logs.open.contains(&all[1]));

    st.view.activity_focus = Some(all[2]);
    key(&mut st, ListKey::Open);
    assert!(st.logs.open.contains(&all[2]), "a group folds by its first entry");
    key(&mut st, ListKey::Toggle);
    assert!(st.logs.open.is_empty());

    st.view.activity_focus = Some(all[1]);
    for k in [ListKey::Delete, ListKey::Across { right: true }, ListKey::Extend(ListMove::Step { down: true })] {
        key(&mut st, k);
    }
    assert!(st.logs.open.is_empty());
    assert_eq!(st.view.activity_focus, Some(all[1]));
}

#[test]
fn keys_do_nothing_off_the_activity_tab_or_with_the_panel_hidden() {
    let mut st = panel(vec![run("Push", true, "out")]);
    st.view.show_log = false;
    key(&mut st, step(true));
    assert_eq!(st.view.activity_focus, None);
    st.view.show_log = true;
    st.logs.tab = LogKind::Errors;
    key(&mut st, step(true));
    assert_eq!(st.view.activity_focus, None);
    st.view.activity_focus = Some(st.log[0].id);
    st.logs.tab = LogKind::Performance;
    key(&mut st, ListKey::Toggle);
    assert!(st.logs.open.is_empty());
}

#[test]
fn filter_state_survives_tab_switches() {
    let mut st = panel(vec![run("Push", false, "out")]);
    let first = st.log[0].id;
    st.tabs[0] = Tab::unloaded(glowfly_core::paths::temp_dir().join("glowfly-logs-filter-a"));
    st.tabs.push(Tab::unloaded(glowfly_core::paths::temp_dir().join("glowfly-logs-filter-b")));
    st.view.logs_search = "out".into();
    send(&mut st, LogsMsg::SetFailuresOnly(true));
    st.view.activity_focus = Some(first);
    send(&mut st, LogsMsg::Show(LogKind::Errors));
    send(&mut st, LogsMsg::Show(LogKind::Activity));
    assert_eq!(st.view.logs_search, "out");
    assert!(st.logs.failures_only);

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert_eq!(st.active_tab, 1);
    assert_eq!(st.view.logs_search, "out");
    assert!(st.logs.failures_only);
    let f = Filter::new(true, "out");
    assert!(!row_ids(&st.log, &f).contains(&first), "the cursor matches no row here");

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(row_ids(&st.log, &f).contains(&first), "and matches again on return");
    assert_eq!(st.view.activity_focus, Some(first));
}

#[test]
fn details_reveals_a_run_the_filter_hid() {
    let mut st = panel(vec![run("Pull", true, "fine"), run("Push", false, "bad")]);
    let (ok, bad) = (st.log[0].id, st.log[1].id);
    let tab = st.tabs[0].id;
    st.view.logs_search = "zzz".into();
    send(&mut st, LogsMsg::SetFailuresOnly(true));
    st.logs.search_closed.insert(ok);
    send(&mut st, LogsMsg::Details { tab, entry: Some(ok) });
    assert!(st.view.logs_search.is_empty());
    assert!(!st.logs.failures_only, "an ok entry turns Failures only off");
    assert!(st.logs.open.contains(&ok));
    assert_eq!(st.view.activity_reveal, Some(ok));
    assert!(st.logs.search_closed.is_empty());

    // A failure under Failures only alone keeps the toggle on.
    send(&mut st, LogsMsg::SetFailuresOnly(true));
    st.view.activity_reveal = None;
    send(&mut st, LogsMsg::Details { tab, entry: Some(bad) });
    assert!(st.logs.failures_only);
    assert_eq!(st.view.activity_reveal, Some(bad));

    // A failure the search hides: the search goes, the toggle stays.
    st.view.logs_search = "zzz".into();
    send(&mut st, LogsMsg::Details { tab, entry: Some(bad) });
    assert!(st.view.logs_search.is_empty());
    assert!(st.logs.failures_only);
}

#[test]
fn clear_forgets_search_closed_rows() {
    let mut st = panel(vec![run("Pull", true, "needle")]);
    let id = st.log[0].id;
    st.logs.search_closed.insert(id);
    update(&mut st, Msg::ClearLog, Instant::now());
    assert!(st.logs.search_closed.is_empty());
}

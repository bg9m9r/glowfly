//! Headless tests for find and go to line in the history's File view: matches and landing,
//! stepping round, the Line field, focusing the Find field, and a new version's fresh start.

use super::history_tests::{oid, with_view};
use super::update;
use crate::effect::Effect;
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::history::{FileTextData, HistoryPane};
use crate::state::history_text::TextNav;
use std::sync::Arc;
use std::time::Instant;

fn send(st: &mut AppState, m: HistoryMsg) -> Vec<Effect> {
    update(st, Msg::History(m), Instant::now())
}

fn text(lines: &[&str]) -> Arc<FileTextData> {
    Arc::new(FileTextData { lines: lines.iter().map(|l| l.to_string()).collect(), spans: None, lfs: Default::default() })
}

fn nav(st: &AppState) -> &TextNav {
    &st.history.as_ref().unwrap().nav
}

fn nav_mut(st: &mut AppState) -> &mut TextNav {
    &mut st.history.as_mut().unwrap().nav
}

/// `with_view` in the File pane, entry 0's text loaded.
fn with_text(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_view(name);
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    let lines = text(&["alpha", "beta foo", "gamma", "Foo delta", "eps", "foo"]);
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(1), path: "a.txt".into(), before: false, result: Ok(lines) });
    (st, dir)
}

#[test]
fn find_lands_on_the_first_match_below_the_top_and_steps_round() {
    let (mut st, dir) = with_text("history-find-step");
    nav_mut(&mut st).top = 2;
    nav_mut(&mut st).find.query = "foo".into();
    send(&mut st, HistoryMsg::FileFindChanged);
    assert_eq!(nav(&st).find.hits, [1, 3, 5]);
    assert_eq!(nav(&st).find.current, Some(1));
    assert_eq!((nav(&st).cursor, nav(&st).scroll_to), (Some(3), Some(3)));
    send(&mut st, HistoryMsg::FileFindStep(1));
    assert_eq!(nav(&st).cursor, Some(5));
    send(&mut st, HistoryMsg::FileFindStep(1));
    assert_eq!(nav(&st).cursor, Some(1), "wraps to the first");
    send(&mut st, HistoryMsg::FileFindStep(-1));
    assert_eq!(nav(&st).cursor, Some(5), "and back to the last");
    nav_mut(&mut st).find.query = "nothing here".into();
    send(&mut st, HistoryMsg::FileFindChanged);
    assert!(nav(&st).find.hits.is_empty() && nav(&st).find.current.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn go_to_line_clamps_and_ignores_other_text() {
    let (mut st, dir) = with_text("history-find-goto");
    nav_mut(&mut st).goto = "2".into();
    send(&mut st, HistoryMsg::FileGoToLine);
    assert_eq!((nav(&st).cursor, nav(&st).scroll_to), (Some(1), Some(1)));
    nav_mut(&mut st).goto = "999".into();
    send(&mut st, HistoryMsg::FileGoToLine);
    assert_eq!(nav(&st).cursor, Some(5), "clamped to the last line");
    nav_mut(&mut st).goto = "abc".into();
    send(&mut st, HistoryMsg::FileGoToLine);
    assert_eq!(nav(&st).cursor, Some(5), "ignored");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn focus_find_only_in_the_file_pane() {
    let (mut st, dir) = with_text("history-find-focus");
    send(&mut st, HistoryMsg::FocusFileFind);
    assert!(nav(&st).focus_find);
    nav_mut(&mut st).focus_find = false;
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::Diff));
    send(&mut st, HistoryMsg::FocusFileFind);
    assert!(!nav(&st).focus_find);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_version_starts_at_the_top_and_keeps_the_query() {
    let (mut st, dir) = with_text("history-find-new");
    nav_mut(&mut st).find.query = "foo".into();
    send(&mut st, HistoryMsg::FileFindChanged);
    nav_mut(&mut st).top = 4;
    nav_mut(&mut st).to_top = false;
    send(&mut st, HistoryMsg::SelectHistoryEntry(1));
    let n = nav(&st);
    assert_eq!((n.cursor, n.scroll_to, n.top, n.to_top), (None, None, 0, true));
    assert_eq!(n.find.query, "foo");
    assert!(n.find.hits.is_empty());
    let lines = text(&["foo", "bar", "more foo"]);
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(2), path: "old.txt".into(), before: false, result: Ok(lines) });
    let n = nav(&st);
    assert_eq!(n.find.hits, [0, 2], "matches found again");
    assert_eq!((n.find.current, n.cursor, n.scroll_to), (None, None, None), "without moving to one");
    let _ = std::fs::remove_dir_all(&dir);
}

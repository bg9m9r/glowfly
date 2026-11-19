//! The activity panel's Failures only toggle and search: the filtered list, the matched text
//! marked in the title, summary, cause and output, the lines nothing left, the list's keyboard
//! cursor and its keys (never while typing in the search field), and a searched Errors tab.

use super::snapshot_tests::{harness, keys_harness, settings};
use super::snapshot_tests_activity::{note, session, tall};
use super::snapshot_tests_painted::fills_at;
use super::snapshot_tests_search_marks::texts;
use crate::activity_keys;
use crate::snapshot_check::CheckSnapshot;
use crate::theme::color;
use eframe::egui::{Color32, Event, Key, Modifiers, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::logs::LogTail;
use glowfly_core::settings::{SizedPanel, ThemeChoice};
use glowfly_model::logs::{LogKind, LogsMsg};
use glowfly_model::msg::{JobMsg, Msg};
use glowfly_model::state::list_keys::{ListKey, ListMove};
use glowfly_model::state::logs::{LogFileView, Persist};
use glowfly_model::state::{AppState, Modal};
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const SUMMARY: &str = "Your changes to 1 file would be overwritten.";
const PUSH: &str = "Push, done, Everything up-to-date";

/// The row keys the frames so far sent.
fn row_keys(h: &H) -> Vec<ListKey> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Logs(LogsMsg::RowKey(k)) => Some(*k),
            _ => None,
        })
        .collect()
}

fn sent(h: &H, want: impl Fn(&LogsMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Logs(l) if want(l)))
}

/// `session` with the Activity panel tall enough for a keyboard harness (1400 x 820).
fn keyed(search: &str) -> (H, glowfly_model::state::logs::EntryId) {
    let (mut st, [push, ..]) = session(ThemeChoice::Dark);
    st.settings.panel_sizes.set(SizedPanel::Activity, 440.0);
    st.view.logs_search = search.into();
    let mut h = keys_harness(st, true);
    h.run();
    (h, push)
}

#[test]
fn activity_failures_only() {
    let (mut st, _) = session(ThemeChoice::Dark);
    st.logs.failures_only = true;
    let mut h = tall(st, true);
    h.check_snapshot("activity_failures_only");
    assert!(h.query_by_label("Push, done, Everything up-to-date").is_none(), "successes are hidden");
    assert!(h.query_by_label("Auto-fetch, done, Ran 5 times").is_none(), "so are the routine groups");
    h.get_by_label("Pull, failed, Your changes to 1 file would be overwritten.");
    h.get_by_label("Couldn't reach github.com, failed");
    h.get_by_label("Failures only").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, LogsMsg::SetFailuresOnly(false))), "the toggle asks update to turn it off");
    assert!(h.state().0.logs.failures_only, "and changes nothing itself");
}

#[test]
fn activity_search_marked() {
    let (mut st, [_, _, pull]) = session(ThemeChoice::Dark);
    st.logs.open.remove(&pull);
    st.view.logs_search = "overwritten".into();
    let mut h = tall(st, true);
    h.check_snapshot("activity_search_marked");
    assert!(!h.state().0.logs.open.contains(&pull), "unfolded by the search, not by `open`");

    let pal = color::current();
    let shapes = texts(&h);
    let mut summaries: Vec<_> = shapes.iter().filter(|t| t.galley.text() == SUMMARY).collect();
    summaries.sort_by(|a, b| a.pos.y.total_cmp(&b.pos.y));
    let [row, cause] = summaries[..] else { panic!("the row's summary and the cause sentence: {}", summaries.len()) };
    let slice =
        |t: &eframe::egui::epaint::TextShape, s: &eframe::egui::text::LayoutSection| t.galley.text()[s.byte_range.clone()].to_owned();

    // The row's summary: the match in full ink on the find fill.
    assert!(row.galley.job.sections.len() > 1, "the summary is split around the match");
    let hit = row.galley.job.sections.iter().find(|s| s.format.color == pal.text && slice(row, s) == "overwritten").expect("inked match");
    let start = SUMMARY.find("overwritten").unwrap();
    let glyph = &row.galley.rows[0].glyphs[SUMMARY[..start].chars().count() + 2];
    let y = row.pos.y + row.galley.size().y / 2.0;
    let under = *fills_at(&h, pos2(row.pos.x + glyph.pos.x, y)).last().expect("a fill under the match");
    assert_eq!(under, color::find_fill(pal.panel));
    assert_eq!(hit.format.background, Color32::TRANSPARENT, "painted as a rounded fill, not a text background");

    // The cause sentence and the output block: a text background on exactly the match.
    let marked = |t: &eframe::egui::epaint::TextShape| -> Vec<String> {
        t.galley.job.sections.iter().filter(|s| s.format.background != Color32::TRANSPARENT).map(|s| slice(t, s)).collect()
    };
    assert_eq!(marked(cause), ["overwritten"]);
    let output = shapes.iter().find(|t| t.galley.text().contains("$ git pull --ff")).expect("the output block");
    assert_eq!(marked(output), ["overwritten"]);
    assert_eq!(output.galley.job.sections.iter().find(|s| s.format.background != Color32::TRANSPARENT).unwrap().format.color, pal.text);
}

#[test]
fn activity_search_routine_run() {
    let (mut st, _) = session(ThemeChoice::Dark);
    let third = &mut st.log[3];
    third.text = "Auto-fetch done\n$ git fetch --all\nFrom example.com:team/glowfly".into();
    st.view.logs_search = "example.com".into();
    let h = tall(st, true);
    assert!(h.query_by_label("Auto-fetch, done, Ran 5 times").is_none(), "no group while searching");
    h.get_by_label("Auto-fetch, done");
    assert!(texts(&h).iter().any(|t| t.galley.text().contains("From example.com:team/glowfly")), "the match's row is unfolded");
    assert!(h.query_by_label("Push, done, Everything up-to-date").is_none());
}

#[test]
fn activity_search_none() {
    let (mut st, _) = session(ThemeChoice::Dark);
    st.view.logs_search = "zzz".into();
    let mut h = tall(st, true);
    h.check_snapshot("activity_search_none");
    h.get_by_label("Nothing in this tab's activity matches “zzz”.");

    h.get_by_label("Clear search").click();
    h.run();
    assert!(h.state().0.view.logs_search.is_empty(), "the clear button empties the field");
    assert!(sent(&h, |m| matches!(m, LogsMsg::SearchChanged)));
    h.get_by_label("Push, done, Everything up-to-date");

    // Failures only on its own, and with a search.
    let (mut st, _) = session(ThemeChoice::Dark);
    st.logs.failures_only = true;
    st.view.logs_search = "zzz".into();
    let h = tall(st, true);
    h.get_by_label("No failure in this tab matches “zzz”.");
    let (mut st, _) = session(ThemeChoice::Dark);
    st.logs.failures_only = true;
    st.log.retain(|e| e.ok);
    let h = tall(st, true);
    h.get_by_label("Nothing has failed in this tab.");

    // An empty log keeps its own line.
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.failures_only = true;
    st.view.logs_search = "zzz".into();
    let h = tall(st, true);
    h.get_by_label("Nothing has happened in this tab yet.");
}

#[test]
fn activity_focused_row() {
    let (mut h, push) = keyed("");
    assert!(!activity_keys::focused(&h.ctx));
    h.get_by_label(PUSH).click();
    h.run();
    assert!(activity_keys::focused(&h.ctx), "the click gave the list the keyboard");
    assert_eq!(h.state().0.view.activity_focus, Some(push), "and made the row its cursor");
    h.check_snapshot("activity_focused_row");

    h.state_mut().1.clear();
    for key in [Key::ArrowDown, Key::ArrowUp, Key::Home, Key::End, Key::PageDown, Key::Space, Key::Enter] {
        h.press_key(key);
    }
    h.run_ok();
    let got = row_keys(&h);
    assert_eq!(got.len(), 7, "{got:?}");
    assert_eq!(
        got[..4],
        [
            ListKey::Move(ListMove::Step { down: true }),
            ListKey::Move(ListMove::Step { down: false }),
            ListKey::Move(ListMove::Edge { end: false }),
            ListKey::Move(ListMove::Edge { end: true }),
        ]
    );
    let ListKey::Move(ListMove::Page { down: true, rows }) = got[4] else { panic!("{:?}", got[4]) };
    assert!(rows >= 1, "a page is the rows in view, less one: {rows}");
    assert_eq!(got[5..], [ListKey::Toggle, ListKey::Open]);

    // Not under a dialog.
    h.state_mut().0.modal = Some(Modal::Shortcuts { filter: String::new() });
    h.run_ok();
    h.state_mut().1.clear();
    for key in [Key::ArrowDown, Key::Space, Key::Enter] {
        h.press_key(key);
    }
    h.run_ok();
    assert!(row_keys(&h).is_empty(), "the sheet covers the list: {:?}", row_keys(&h));
}

#[test]
fn typing_in_the_search_field_leaves_the_rows_alone() {
    let (mut h, _) = keyed("");
    h.get_by_label("Search titles, causes and output").focus();
    h.run();
    assert!(h.get_by_label("Search titles, causes and output").is_focused());
    assert!(!activity_keys::focused(&h.ctx));
    h.state_mut().1.clear();
    for key in [Key::ArrowDown, Key::Space, Key::Enter] {
        h.press_key(key);
    }
    h.run_ok();
    assert!(row_keys(&h).is_empty(), "the field has the keys: {:?}", row_keys(&h));

    h.get_by_label("Search titles, causes and output").focus();
    h.run();
    h.get_by_label("Search titles, causes and output").type_text("x");
    h.run_ok();
    assert_eq!(h.state().0.view.logs_search, "x");
    assert!(sent(&h, |m| matches!(m, LogsMsg::SearchChanged)));
    assert!(row_keys(&h).is_empty());

    // A focused activity list is not typing: a binding that waits for it still fires.
    let (mut h, _) = keyed("");
    h.get_by_label(PUSH).click();
    h.run();
    assert!(activity_keys::focused(&h.ctx));
    h.state_mut().1.clear();
    let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    h.input_mut().modifiers = ctrl;
    h.input_mut().events.push(Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers: ctrl });
    h.run_ok();
    h.input_mut().modifiers = Modifiers::NONE;
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Undo))), "Undo is not blocked by the list's focus");
}

#[test]
fn the_cursor_row_scrolls_into_view() {
    let (mut st, _) = session(ThemeChoice::Dark);
    for n in 0..30 {
        st.log.push(note(true, &format!("Checked out topic-{n}"), 0));
    }
    st.logs.persist = Persist::On;
    let first = st.log.iter().find(|e| e.text == "Checked out topic-0").unwrap().id;
    let mut h = harness(st, true);
    h.run();
    let row = "Checked out topic-0, done";
    let bounds = |h: &H, label: &str| h.get_by_label(label).bounding_box().expect("laid out");
    let list_top = bounds(&h, "Activity").y1;
    assert!(bounds(&h, row).y0 < list_top, "stuck to the bottom, the early row is above the list");
    h.state_mut().0.view.activity_focus = Some(first);
    h.state_mut().0.view.activity_focus_scroll = true;
    h.run();
    let b = bounds(&h, row);
    assert!(b.y0 >= list_top && b.y1 <= 820.0, "the cursor row is in the list: {b:?} under {list_top}");
    assert!(!h.state().0.view.activity_focus_scroll, "the view cleared the request");
    h.run();
    h.run();
    assert_eq!(bounds(&h, row).y0, b.y0, "and the list stays there");
}

/// The error tab's fixture: a truncated tail with an ERROR, a WARN and two INFO lines.
fn error_tab(search: &str) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    st.view.show_log = true;
    st.logs.tab = LogKind::Errors;
    let lines = [
        "2026-10-16 09:30:01.120 +00:00 ERROR glowfly-load-repo glowfly::runner::guard: effect panicked: index out of bounds",
        "2026-10-16 09:30:04.481 +00:00  WARN main glowfly::themes: theme file sand-dark.json: unknown color \"glow\"",
        "2026-10-16 09:31:12.007 +00:00  INFO glowfly-git glowfly::runner: fetch finished",
        "2026-10-16 09:31:12.010 +00:00  INFO main glowfly::ui: window focused",
    ];
    let tail = LogTail { lines: lines.iter().map(|l| l.to_string()).collect(), truncated: true };
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Ok(tail)) });
    st.view.logs_search = search.into();
    st
}

#[test]
fn activity_log_error_tab_search() {
    let mut h = harness(error_tab("theme"), false);
    h.run();
    h.check_snapshot("activity_log_error_tab_search");
    let shapes = texts(&h);
    let line = shapes.iter().find(|t| t.galley.text().contains("WARN main")).expect("the WARN line");
    let marked: Vec<_> = line
        .galley
        .job
        .sections
        .iter()
        .filter(|s| s.format.background != Color32::TRANSPARENT)
        .map(|s| line.galley.text()[s.byte_range.clone()].to_owned())
        .collect();
    assert_eq!(marked, ["theme", "theme"], "both hits in the line are marked (the module and the file)");
    assert!(shapes.iter().all(|t| !t.galley.text().contains("INFO")), "the other lines are filtered out");
    assert!(h.query_by_label("Failures only").is_none(), "the toggle is the Activity tab's");

    h.state_mut().0.view.logs_search = "zzzz".into();
    h.run();
    h.get_by_label("No line shown here matches “zzzz”.");
}

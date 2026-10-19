//! Terminal tabs in the panel's header: two shells of one repository with the second active,
//! their close crosses, New terminal tab and Kill all; and the toolbar Terminal button's
//! right-click item, which follows Preferences > Terminal.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_terminal::{pushed, with_session};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::{SessionStatus, TermSession};
use crate::terminal::TerminalMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::widgets::tab_glyph;
use eframe::egui::{PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::terminal::TermScreen;

/// Two shells of the fixture repository, the second active.
fn two_sessions(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_session(ThemeChoice::Dark, name, SessionStatus::Running);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let mut screen = TermScreen::new(24, 80);
    screen.feed(b"PS C:\\work\\demo> cargo test\r\n   Compiling demo v0.1.0\r\n");
    st.terminal.sessions.push(TermSession {
        id: 2,
        number: 2,
        selection: None,
        workdir: workdir.clone(),
        program: "pwsh".into(),
        screen,
        status: SessionStatus::Running,
    });
    st.terminal.active.insert(workdir, 2);
    st.terminal.next_id = 3;
    (st, dir)
}

/// The bounds of the widget labelled `label`.
fn bounds(h: &Harness<'_, (AppState, Vec<Msg>)>, label: &str) -> Rect {
    let b = h.get_by_label(label).bounding_box().unwrap_or_else(|| panic!("{label} has bounds"));
    Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

#[test]
fn terminal_tabs_dark() {
    let (st, dir) = two_sessions("terminal-tabs");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_tabs");
    for label in ["Terminal 1", "Terminal 2", "Close Terminal 1", "New terminal tab", "Kill all"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.get_by_label("Terminal 1").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::SelectTab(1))));
    h.get_by_label("Close Terminal 2").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::CloseTab(2))));
    h.get_by_label("New terminal tab").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::NewTab)));
    h.get_by_label("Kill all").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::AskKillAll)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn one_terminal_shows_no_strip_but_new_tab() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-one-tab", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("New terminal tab").is_some());
    assert!(h.query_by_label("Terminal 1").is_none());
    assert!(h.query_by_label("Kill all").is_none(), "one shell alive");
    let (plus, hide) = (bounds(&h, "New terminal tab"), bounds(&h, "Hide"));
    assert!((plus.center().y - hide.center().y).abs() <= 1.0, "+ at {plus:?}, Hide at {hide:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_right_click_follows_the_setting() {
    for (in_panel, item) in [(false, "Open in external terminal"), (true, "New terminal tab")] {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, if in_panel { "terminal-button-panel" } else { "terminal-button-external" });
        st.settings.terminal.open_in_panel = in_panel;
        let mut h = harness(st, true);
        h.run();
        let b = h.get_by_label("Show the terminal panel").bounding_box().expect("the toolbar button");
        click(&mut h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Secondary);
        h.get_by_label(item).click();
        h.run();
        assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenTerminal)), "{item}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn strip_lines_up_with_the_header() {
    let (st, dir) = two_sessions("terminal-tabs-line");
    let mut h = harness(st, true);
    h.run();
    let kill_all = bounds(&h, "Kill all");
    for label in ["Terminal 2", "Close Terminal 2", "New terminal tab"] {
        let b = bounds(&h, label);
        assert!((b.center().y - kill_all.center().y).abs() <= 1.0, "{label} at {b:?}, Kill all at {kill_all:?}");
    }
    let (plus, close) = (bounds(&h, "New terminal tab"), bounds(&h, "Close Terminal 2"));
    assert_eq!(plus.size(), close.size(), "the + and the × have one size");
    assert_eq!(plus.size(), vec2(tab_glyph::HIT, tab_glyph::HIT));
    h.get_by_label("New terminal tab").hover();
    h.run();
    h.check_snapshot("terminal_tabs_plus_hover");
    h.get_by_label("Close Terminal 1").hover();
    h.run();
    h.check_snapshot("terminal_tabs_close_hover");
    drop(h);
    remove_fixture(&dir);
}

//! The terminal grid's history, selection and menu: scrolled back with a selection and the
//! thumb, the wheel, dragging to select (and Copy on select), copying with the keyboard,
//! middle-click paste, the right-click menu, and the Bar and Underline cursors with dimming.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_select::click;
use super::snapshot_tests_terminal::{pushed, with_session};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::{Selection, SessionStatus};
use crate::terminal::TerminalMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::{CursorStyle, ThemeChoice};
use trellis_core::terminal::{TermPoint, TermScreen};

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// A running session that has printed "line 1" to "line 84": 60 rows of history.
fn with_history(theme: ThemeChoice, name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_session(theme, name, SessionStatus::Running);
    let mut screen = TermScreen::new(24, 80);
    let lines: Vec<String> = (1..=84).map(|i| format!("line {i}")).collect();
    screen.feed(format!("{}\r\nPS> ", lines.join("\r\n")).as_bytes());
    st.terminal.sessions[0].screen = screen;
    (st, dir)
}

/// The middle of the grid's cell (row, col), from the grid's bounds.
fn cell(h: &H, row: u16, col: u16) -> Pos2 {
    let b = h.get_by_label("Terminal screen").bounding_box().expect("the grid");
    let st = &h.state().0;
    let font = eframe::egui::FontId::monospace(st.settings.terminal.font_px());
    let (cw, rh) = h.ctx.fonts(|f| super::terminal::metrics::cell_size(f, &font));
    pos2(b.x0 as f32 + 4.0 + (f32::from(col) + 0.5) * cw, b.y0 as f32 + 4.0 + (f32::from(row) + 0.5) * rh)
}

fn wheel(h: &mut H, at: Pos2, modifiers: Modifiers) {
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, 1.0), modifiers });
    h.run();
}

#[test]
fn terminal_scrolled_dark() {
    let (mut st, dir) = with_history(ThemeChoice::Dark, "terminal-scrolled");
    let s = &mut st.terminal.sessions[0];
    s.screen.scroll_by(10);
    let top = s.screen.top_line();
    let (anchor, head) = (TermPoint { line: top + 2, col: 3 }, TermPoint { line: top + 4, col: 6 });
    s.selection = Some(Selection { anchor, head, epoch: s.screen.epoch() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_scrolled");

    let mid = cell(&h, 5, 10);
    wheel(&mut h, mid, Modifiers::NONE);
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Scroll(3))), "a notch is three rows");
    wheel(&mut h, mid, Modifiers::CTRL);
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Scroll(_))), "Ctrl+wheel is the app's zoom");

    // Ctrl+C (Cmd+C on macOS) with a selection copies it and sends nothing to the shell.
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Copy);
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::CopySelection { keep: false })));
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Input(_))));
    drop(h);
    remove_fixture(&dir);
}

/// Drags the left button from cell (2, 1) to cell (4, 7).
fn drag(h: &mut H) {
    let (from, to) = (cell(h, 2, 1), cell(h, 4, 7));
    let press = |h: &mut H, pos: Pos2, pressed: bool| {
        h.input_mut().events.push(Event::PointerMoved(pos));
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    };
    press(h, from, true);
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        h.input_mut().events.push(Event::PointerMoved(at));
        h.step();
    }
    press(h, to, false);
    h.run();
}

#[test]
fn dragging_selects_text() {
    let (mut st, dir) = with_history(ThemeChoice::Dark, "terminal-drag");
    st.terminal.sessions[0].selection = None;
    let mut h = harness(st, true);
    h.run();
    drag(&mut h);
    let s = &h.state().0.terminal.sessions[0];
    let sel = s.selection.expect("dragging selected text");
    let top = s.screen.top_line();
    assert_eq!(sel.anchor.line, top + 2);
    assert_eq!(sel.head.line, top + 4);
    assert!(sel.range(&s.screen).is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn copy_on_select_copies_when_the_drag_ends() {
    for on in [true, false] {
        let (mut st, dir) = with_history(ThemeChoice::Dark, &format!("terminal-copy-select-{on}"));
        st.terminal.sessions[0].selection = None;
        st.settings.terminal.copy_on_select = on;
        let mut h = harness(st, true);
        h.run();
        h.state_mut().1.clear();
        drag(&mut h);
        assert_eq!(pushed(&h, |t| matches!(t, TerminalMsg::CopySelection { keep: true })), on, "copy on select {on}");
        assert!(h.state().0.terminal.sessions[0].selection.is_some(), "the selection stays");
        // With it on, the copy key no longer copies: the text was copied when it was selected.
        h.state_mut().1.clear();
        h.input_mut().events.push(Event::Copy);
        h.run();
        assert_eq!(pushed(&h, |t| matches!(t, TerminalMsg::CopySelection { keep: false })), !on, "copy on select {on}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn middle_click_pastes_on_linux() {
    for on in [true, false] {
        let (mut st, dir) = with_history(ThemeChoice::Dark, &format!("terminal-middle-{on}"));
        st.settings.terminal.middle_click_paste = on;
        let mut h = harness(st, true);
        h.run();
        h.state_mut().1.clear();
        let at = cell(&h, 6, 12);
        click(&mut h, at, PointerButton::Middle);
        let want = on && cfg!(target_os = "linux");
        assert_eq!(pushed(&h, |t| matches!(t, TerminalMsg::Paste)), want, "middle-click paste {on}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn terminal_context_menu_dark() {
    let (st, dir) = with_history(ThemeChoice::Dark, "terminal-menu");
    let mut h = harness(st, true);
    h.run();
    let at = cell(&h, 6, 12);
    click(&mut h, at, PointerButton::Secondary);
    h.check_snapshot("terminal_context_menu");
    assert!(h.get_by_label("Copy").is_disabled(), "nothing is selected");
    for (label, want) in [
        ("Paste", (|t: &TerminalMsg| matches!(t, TerminalMsg::Paste)) as fn(&TerminalMsg) -> bool),
        ("Select all", |t| matches!(t, TerminalMsg::SelectAll)),
        ("Clear", |t| matches!(t, TerminalMsg::Clear)),
    ] {
        if h.query_by_label(label).is_none() {
            click(&mut h, at, PointerButton::Secondary);
        }
        h.get_by_label(label).click();
        h.run();
        assert!(pushed(&h, want), "{label}");
    }

    // With a selection, Copy keeps it.
    let s = &mut h.state_mut().0.terminal.sessions[0];
    let top = s.screen.top_line();
    s.selection =
        Some(Selection { anchor: TermPoint { line: top, col: 0 }, head: TermPoint { line: top + 1, col: 4 }, epoch: s.screen.epoch() });
    click(&mut h, at, PointerButton::Secondary);
    h.get_by_label("Copy").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::CopySelection { keep: true })));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_bar_dimmed_light() {
    let (mut st, dir) = with_session(ThemeChoice::Light, "terminal-bar", SessionStatus::Running);
    st.settings.terminal.cursor = CursorStyle::Bar;
    st.settings.terminal.dim_unfocused = true;
    st.terminal.focus_requested = false;
    let mut h = harness(st, false);
    h.run();
    assert!(!h.state().0.terminal.focused);
    h.check_snapshot("terminal_bar_dimmed_light");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-underline", SessionStatus::Running);
    st.settings.terminal.cursor = CursorStyle::Underline;
    let mut h = harness(st, true);
    h.run();
    assert!(h.state().0.terminal.focused);
    assert!(h.query_by_label("Terminal screen").is_some());
    drop(h);
    remove_fixture(&dir);
}

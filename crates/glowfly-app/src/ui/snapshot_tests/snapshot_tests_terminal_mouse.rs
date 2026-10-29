//! The terminal grid's word and line clicks, scrolling the history by dragging a selection past
//! the grid's edge, and the wheel in a full-screen program (arrow keys, or nothing while the
//! program asked for mouse events).

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_select::double_click;
use super::snapshot_tests_terminal::{pushed, with_session};
use super::snapshot_tests_terminal_select::{H, cell, with_history};
use crate::state::terminal::SessionStatus;
use crate::terminal::TerminalMsg;
use eframe::egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::terminal::{KeyMods, TermInput, TermKey};

/// The text of session 0's selection.
fn selected(h: &mut H) -> String {
    let s = &mut h.state_mut().0.terminal.sessions[0];
    let (a, b) = s.selection.and_then(|sel| sel.range(&s.screen)).expect("a selection");
    s.screen.text_between(a, b)
}

/// Three primary clicks at `pos` in one frame.
fn triple_click(h: &mut H, pos: Pos2) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    for pressed in [true, false, true, false, true, false] {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_ok();
}

#[test]
fn double_and_triple_clicks_select_a_word_and_a_line() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-word", SessionStatus::Running);
    st.settings.terminal.copy_on_select = true;
    let mut h = harness(st, true);
    h.run();
    // "PS C:\work\demo> git status": the path, without the prompt's ">".
    let at = cell(&h, 0, 8);
    double_click(&mut h, at, Modifiers::NONE);
    assert_eq!(selected(&mut h), "C:\\work\\demo");
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Selected)), "reported (PRIMARY, and copied with Copy on select)");

    h.state_mut().1.clear();
    let at = cell(&h, 3, 22);
    triple_click(&mut h, at);
    assert_eq!(selected(&mut h), "        modified:   README.md", "the whole line");
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Selected)));

    // A single click drops the selection.
    let at = cell(&h, 5, 3);
    super::snapshot_tests_select::click(&mut h, at, PointerButton::Primary);
    assert!(h.state().0.terminal.sessions[0].selection.is_none());
    drop(h);
    remove_fixture(&dir);
}

/// Presses the left button on cell (row, col), drags to `to` and holds it there for `frames`.
fn drag_to(h: &mut H, row: u16, col: u16, to: Pos2, frames: usize) {
    let from = cell(h, row, col);
    h.input_mut().events.push(Event::PointerMoved(from));
    h.input_mut().events.push(Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    for i in 1..=4 {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * (i as f32 / 4.0)));
        h.step();
    }
    for _ in 0..frames {
        h.step();
    }
}

fn scrolled(h: &H) -> i32 {
    h.state().1.iter().filter_map(|m| if let crate::msg::Msg::Terminal(TerminalMsg::Scroll(n)) = m { Some(*n) } else { None }).sum()
}

#[test]
fn dragging_past_the_edge_scrolls_the_history() {
    let (st, dir) = with_history(ThemeChoice::Dark, "terminal-edge-drag");
    let mut h = harness(st, true);
    h.run();
    let grid = h.get_by_label("Terminal screen").bounding_box().expect("the grid");
    let (top, bottom) = (grid.y0 as f32, grid.y1 as f32);

    h.state_mut().1.clear();
    drag_to(&mut h, 4, 3, pos2(200.0, top - 10.0), 3);
    let near = scrolled(&h);
    assert!(near > 0, "held above the grid, it scrolls back ({near})");
    let sel = h.state().0.terminal.sessions[0].selection.expect("still selecting");
    assert_eq!(sel.head.line, h.state().0.terminal.sessions[0].screen.top_line(), "the head follows the top row");
    h.input_mut().events.push(Event::PointerButton {
        pos: pos2(200.0, top - 10.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();

    h.state_mut().1.clear();
    drag_to(&mut h, 4, 3, pos2(200.0, top - 120.0), 3);
    let far = scrolled(&h);
    assert!(far > near, "further out scrolls faster ({far} rows, {near} near the edge)");
    h.input_mut().events.push(Event::PointerButton {
        pos: pos2(200.0, top - 120.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();

    h.state_mut().1.clear();
    drag_to(&mut h, 4, 3, pos2(200.0, bottom + 2.0), 3);
    assert!(scrolled(&h) < 0, "below the grid, it scrolls forward");
    drop(h);
    remove_fixture(&dir);
}

fn wheel(h: &mut H, at: Pos2, notches: f32) {
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, notches), modifiers: Modifiers::NONE });
    h.run();
}

fn sent_keys(h: &H) -> Vec<TermInput> {
    h.state()
        .1
        .iter()
        .filter_map(|m| if let crate::msg::Msg::Terminal(TerminalMsg::Input(v)) = m { Some(v.clone()) } else { None })
        .flatten()
        .collect()
}

#[test]
fn the_wheel_sends_arrows_to_a_full_screen_program() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-alt-wheel", SessionStatus::Running);
    st.terminal.sessions[0].screen.feed(b"\x1b[?1049h\x1b[Hlog page");
    let mut h = harness(st, true);
    h.run();
    let at = cell(&h, 5, 10);
    let key = |key| TermInput::Key { key, mods: KeyMods::default() };
    wheel(&mut h, at, 1.0);
    assert_eq!(sent_keys(&h), vec![key(TermKey::Up); 3], "a notch up is three Up arrows");
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Scroll(_))), "the alternate screen has no history");
    wheel(&mut h, at, -1.0);
    assert_eq!(sent_keys(&h), vec![key(TermKey::Down); 3]);

    // A program that asked for mouse events gets no arrows.
    h.state_mut().0.terminal.sessions[0].screen.feed(b"\x1b[?1000h");
    wheel(&mut h, at, 1.0);
    assert!(sent_keys(&h).is_empty());
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Scroll(_))));
    drop(h);
    remove_fixture(&dir);
}

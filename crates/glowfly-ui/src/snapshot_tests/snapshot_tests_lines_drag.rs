//! Selecting diff lines by press-and-drag in their gutter, driven through the real view with
//! pointer events: the messages the rows send, and the selection they lead to, inline and split;
//! and a drag across the code, which picks no lines.

use super::super::snapshot_tests::{harness, remove_fixture};
use super::super::snapshot_tests_revert::with_line_diff;
use super::with_diff;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Runs the frames for the events queued, then applies the diff messages the view sent and
/// returns them.
fn frame(h: &mut H) -> Vec<DiffMsg> {
    h.run();
    let (st, msgs) = h.state_mut();
    let mut sent = Vec::new();
    for m in std::mem::take(msgs) {
        if let Msg::Diff(d) = m {
            let copy = match &d {
                DiffMsg::StartLineDrag { hunk, line, how } => DiffMsg::StartLineDrag { hunk: *hunk, line: *line, how: *how },
                DiffMsg::DragDiffLine { hunk, line } => DiffMsg::DragDiffLine { hunk: *hunk, line: *line },
                DiffMsg::EndLineDrag => DiffMsg::EndLineDrag,
                DiffMsg::SelectDiffLine { hunk, line, how } => DiffMsg::SelectDiffLine { hunk: *hunk, line: *line, how: *how },
                DiffMsg::ClearLineSelection => DiffMsg::ClearLineSelection,
                _ => continue,
            };
            assert!(crate::update::update(st, Msg::Diff(d), Instant::now()).is_empty());
            sent.push(copy);
        }
    }
    sent
}

fn button(h: &mut H, pos: Pos2, pressed: bool) {
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
}

/// Drags from `from` to `to` in a few steps, leaving the button down, and returns every diff
/// message sent on the way.
fn drag(h: &mut H, from: Pos2, to: Pos2) -> Vec<DiffMsg> {
    h.input_mut().events.push(Event::PointerMoved(from));
    button(h, from, true);
    let mut sent = frame(h);
    for t in [0.1, 0.2, 0.4, 0.7, 1.0] {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * t));
        sent.extend(frame(h));
    }
    sent
}

/// The left edge of the diff's rows: the sidebar's edge (the rows have no widget of their own).
const ROWS_LEFT: f32 = 261.0;

/// Drags in `layout` from the `-fix` row's gutter (at x `from_x` right of the rows' left edge)
/// down to the `+new line` row (at `to_x` left of the hunk's right end), checks the messages and
/// selection (what the gutter tests' snapshots show), and releases.
fn drag_test(layout: DiffLayout, from_x: f32, to_x: f32) {
    let (st, dir) = with_diff(layout, "diff-drag");
    let lines = &st.session.diff.view.as_ref().unwrap().1.hunks[0].lines;
    let at = |origin: char, text: &str| lines.iter().position(|l| l.origin == origin && l.text == text).unwrap();
    let (fix, new_line) = (at('-', "fix"), at('+', "new line"));
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    // Inline, line i of the hunk is i + 1 rows below the header, which holds the hunk buttons.
    // Split, `-fix` pairs with `+fixed build`, so `+new line` is a row higher.
    let b = h.get_by_label("Stage hunk").bounding_box().expect("the button has bounds");
    let header_y = (b.y0 + b.y1) as f32 / 2.0;
    let from = pos2(ROWS_LEFT + from_x, header_y + (fix + 1) as f32 * row_h);
    let new_row = if layout == DiffLayout::Split { new_line } else { new_line + 1 };
    let to = pos2(b.x1 as f32 - to_x, header_y + new_row as f32 * row_h);
    let mut sent = drag(&mut h, from, to);
    // One more frame draws the last selection (the button still down).
    sent.extend(frame(&mut h));
    // A drag from the gutter adds to the selection, as a gutter click does.
    assert!(
        sent.iter().any(|m| matches!(m, DiffMsg::StartLineDrag { hunk: 0, line, how: LineClick::Toggle } if *line == fix)),
        "the drag starts on -fix"
    );
    assert!(sent.iter().any(|m| matches!(m, DiffMsg::DragDiffLine { hunk: 0, line } if *line == new_line)), "and reaches +new line");
    let sel = &h.state().0.view.diff_lines;
    let lines = &h.state().0.session.diff.view.as_ref().unwrap().1.hunks[0].lines;
    let changed: Vec<usize> = (fix..=new_line).filter(|&i| matches!(lines[i].origin, '+' | '-')).collect();
    // The hunk's lines 2 to 4: `-fix`, `+fixed build` and `+new line`.
    assert_eq!(changed, [1, 2, 3]);
    assert_eq!(sel.lines.iter().copied().collect::<Vec<_>>(), changed);
    assert!(sel.dragging().is_some());

    button(&mut h, to, false);
    let sent = frame(&mut h);
    assert!(sent.iter().any(|m| matches!(m, DiffMsg::EndLineDrag)), "the release ends the drag");
    let sel = &h.state().0.view.diff_lines;
    assert!(sel.dragging().is_none());
    assert_eq!(sel.lines.iter().copied().collect::<Vec<_>>(), changed, "the selection stays");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_gutter_drag_selects_lines() {
    drag_test(DiffLayout::Inline, 100.0, 300.0);
}

#[test]
fn diff_gutter_drag_selects_lines_split() {
    // Split: from the left `-fix` cell's gutter to the right `+new line` cell, the row below it.
    drag_test(DiffLayout::Split, 54.0, 200.0);
}

/// Drags in `layout` up across the code of a commit diff, from its last changed line's text to
/// its first one's (in split, the row above holds the last), and checks that no line is picked
/// on the way or at the release, and that the diff, taller than the view, doesn't scroll.
fn code_drag_test(layout: DiffLayout, name: &str) {
    // ` app`, `-fix`, `+fix the build`, `+on Windows`, none selected, then enough added lines
    // to scroll.
    let (mut st, dir) = with_line_diff(name, ThemeChoice::Dark, layout, &[]);
    let (target, d) = st.session.diff.view.as_ref().unwrap();
    let mut d = (**d).clone();
    for n in 4..120 {
        d.hunks[0].lines.push(glowfly_core::repo::DiffLine { origin: '+', old: None, new: Some(n), text: format!("more {n}") });
    }
    d.intraline = Default::default();
    st.session.diff.view = Some((target.clone(), d.into()));
    let (first, last) = (1, 3);
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("Revert hunk").bounding_box().expect("the button has bounds");
    let header_y = (b.y0 + b.y1) as f32 / 2.0;
    // Inline the text starts 110 px into the row, split 62 px into each cell; 200 px in is
    // well inside it either way.
    let (from, to) =
        (pos2(ROWS_LEFT + 200.0, header_y + (first + 1) as f32 * row_h), pos2(ROWS_LEFT + 260.0, header_y + (last + 1) as f32 * row_h));
    let mut sent = drag(&mut h, to, from);
    button(&mut h, from, false);
    sent.extend(frame(&mut h));
    let after = h.get_by_label("Revert hunk").bounding_box().expect("the button has bounds");
    assert_eq!((after.y0, after.y1), (b.y0, b.y1), "a drag on the code doesn't scroll the diff");
    let picked = sent.iter().filter(|m| matches!(m, DiffMsg::StartLineDrag { .. } | DiffMsg::SelectDiffLine { .. })).count();
    assert_eq!(picked, 0, "a drag across the code picks no lines");
    assert!(h.state().0.view.diff_lines.is_empty());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_code_drag_selects_no_lines() {
    code_drag_test(DiffLayout::Inline, "code-drag");
}

#[test]
fn diff_code_drag_selects_no_lines_split() {
    code_drag_test(DiffLayout::Split, "code-drag-split");
}

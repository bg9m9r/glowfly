//! Selecting diff lines by press-and-drag, driven through the real view with pointer events:
//! the messages the rows send, and the selection they lead to, inline and split.

use super::super::snapshot_tests::{harness, remove_fixture};
use super::with_diff;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::DiffLayout;
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

/// Drags in `layout` from the `-fix` row (at x `from_x` left of the hunk's right end) down to
/// the `+new line` row (at `to_x`), checks the messages and selection (what the gutter tests'
/// snapshots show), and releases.
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
    let from = pos2(b.x1 as f32 - from_x, header_y + (fix + 1) as f32 * row_h);
    let new_row = if layout == DiffLayout::Split { new_line } else { new_line + 1 };
    let to = pos2(b.x1 as f32 - to_x, header_y + new_row as f32 * row_h);
    let mut sent = drag(&mut h, from, to);
    // One more frame draws the last selection (the button still down).
    sent.extend(frame(&mut h));
    assert!(
        sent.iter().any(|m| matches!(m, DiffMsg::StartLineDrag { hunk: 0, line, how: LineClick::Only } if *line == fix)),
        "the drag starts on -fix"
    );
    assert!(sent.iter().any(|m| matches!(m, DiffMsg::DragDiffLine { hunk: 0, line } if *line == new_line)), "and reaches +new line");
    let sel = &h.state().0.view.diff_lines;
    let lines = &h.state().0.session.diff.view.as_ref().unwrap().1.hunks[0].lines;
    let changed: Vec<usize> = (fix..=new_line).filter(|&i| matches!(lines[i].origin, '+' | '-')).collect();
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
fn diff_drag_selects_lines() {
    drag_test(DiffLayout::Inline, 300.0, 300.0);
}

#[test]
fn diff_drag_selects_lines_split() {
    // Split: from the left `-fix` cell to the right `+new line` cell, the row below it.
    drag_test(DiffLayout::Split, 600.0, 200.0);
}

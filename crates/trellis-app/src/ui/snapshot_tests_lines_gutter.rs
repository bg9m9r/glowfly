//! Picking diff lines from the gutter, driven through the real view with pointer events: a
//! click on a changed line's numbers or sign adds it to the selection or takes it out, a click
//! on its text selects only it, and the gutter shows the check boxes (hollow on hover, filled
//! when selected), inline and split, dark and light.

use super::super::snapshot_tests::{harness, remove_fixture};
use super::with_diff;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::settings::DiffLayout;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The hunk's lines: ` app`, `-fix`, `+fixed build`, `+new line`.
const FIX: usize = 1;
const FIXED: usize = 2;
const NEW: usize = 3;

/// Runs the frames for the events queued, applies the diff messages the view sent and returns
/// how each line click was meant.
fn frame(h: &mut H) -> Vec<(usize, LineClick)> {
    h.run();
    let (st, msgs) = h.state_mut();
    let mut clicks = Vec::new();
    for m in std::mem::take(msgs) {
        if let Msg::Diff(d) = m {
            if let DiffMsg::SelectDiffLine { line, how, .. } = &d {
                clicks.push((*line, *how));
            }
            crate::update::update(st, Msg::Diff(d), Instant::now());
        }
    }
    clicks
}

/// A plain click at `at`; returns how the view meant it, with the result drawn.
fn click(h: &mut H, at: Pos2) -> Vec<(usize, LineClick)> {
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    h.input_mut().events.extend([Event::PointerMoved(at), button(true)]);
    let mut clicks = frame(h);
    h.input_mut().events.push(button(false));
    clicks.extend(frame(h));
    // Draw what the clicks led to.
    h.run();
    clicks
}

fn hover(h: &mut H, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    frame(h);
    frame(h);
}

fn selection(h: &H) -> Vec<usize> {
    h.state().0.view.diff_lines.lines.iter().copied().collect()
}

/// Where to point at a line: its gutter (the sign column) and its text.
struct Spots {
    gutter: Box<dyn Fn(usize) -> Pos2>,
    text: Box<dyn Fn(usize) -> Pos2>,
}

/// The spots of `layout`, measured from the hunk header's Stage hunk button (the row's right
/// end) since the rows' left edge has no widget of its own. Split: the removed line is in the
/// left column, the added ones in the right; `-fix` pairs with `+fixed build`.
fn spots(h: &H, layout: DiffLayout) -> Spots {
    let b = h.get_by_label("Stage hunk").bounding_box().expect("the button has bounds");
    let header_y = (b.y0 + b.y1) as f32 / 2.0;
    let row_h = crate::ui::diff::DiffStyle::new(&h.state().0.settings).row_h;
    // The rows run from the sidebar's edge to just right of the button.
    let left = 261.0;
    let right = b.x1 as f32 + 8.0;
    match layout {
        DiffLayout::Inline => {
            let y = move |i: usize| header_y + (i + 1) as f32 * row_h;
            Spots { gutter: Box::new(move |i| pos2(left + 100.0, y(i))), text: Box::new(move |i| pos2(left + 200.0, y(i))) }
        }
        DiffLayout::Split => {
            let half = ((right - left) / 2.0).round();
            let row = |i: usize| if i == NEW { 3 } else { 2 };
            let col = move |i: usize| if i == FIX { left } else { left + half };
            let y = move |i: usize| header_y + row(i) as f32 * row_h;
            Spots { gutter: Box::new(move |i| pos2(col(i) + 54.0, y(i))), text: Box::new(move |i| pos2(col(i) + 150.0, y(i))) }
        }
    }
}

/// Toggles lines from the gutter and replaces them from the text, checking each click and the
/// selection; snapshots a gutter hovered beside a selected line as `hover_name` and three
/// selected lines as `selected_name`.
fn gutter_test(layout: DiffLayout, dark: bool, hover_name: Option<&str>, selected_name: &str) {
    let (st, dir) = with_diff(layout, &format!("diff-gutter-{dark}"));
    let mut h = harness(st, dark);
    h.run();
    let at = spots(&h, layout);
    assert_eq!(click(&mut h, (at.gutter)(FIX)), [(FIX, LineClick::Toggle)], "a gutter click toggles");
    assert_eq!(click(&mut h, (at.gutter)(NEW)), [(NEW, LineClick::Toggle)]);
    assert_eq!(selection(&h), [FIX, NEW], "and keeps the lines selected before");
    assert!(h.query_by_label("Stage 2 lines").is_some(), "the hunk header counts them");
    assert_eq!(click(&mut h, (at.gutter)(FIX)), [(FIX, LineClick::Toggle)]);
    assert_eq!(selection(&h), [NEW], "a second gutter click takes the line out");
    assert_eq!(click(&mut h, (at.text)(FIXED)), [(FIXED, LineClick::Only)], "a click on the text");
    assert_eq!(selection(&h), [FIXED], "selects only that line");
    if let Some(name) = hover_name {
        hover(&mut h, (at.gutter)(NEW));
        h.check_snapshot(name);
    }
    click(&mut h, (at.gutter)(FIX));
    click(&mut h, (at.gutter)(NEW));
    assert_eq!(selection(&h), [FIX, FIXED, NEW]);
    assert!(h.query_by_label("Stage 3 lines").is_some());
    // The pointer off the rows, so only the selection shows.
    hover(&mut h, pos2(700.0, 600.0));
    h.check_snapshot(selected_name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_gutter_toggles_lines_inline_dark() {
    gutter_test(DiffLayout::Inline, true, Some("diff_lines_gutter_hover_dark"), "diff_lines_gutter_selected_dark");
}

#[test]
fn diff_gutter_toggles_lines_inline_light() {
    gutter_test(DiffLayout::Inline, false, Some("diff_lines_gutter_hover_light"), "diff_lines_gutter_selected_light");
}

#[test]
fn diff_gutter_toggles_lines_split() {
    gutter_test(DiffLayout::Split, true, None, "diff_lines_gutter_split");
}

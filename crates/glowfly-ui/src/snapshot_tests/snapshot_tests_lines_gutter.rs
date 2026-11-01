//! Picking diff lines from the gutter, driven through the real view with pointer events: a
//! click on a changed line's numbers or sign adds it to the selection or takes it out, a click
//! on its text picks no line, and the gutter shows the check boxes (hollow on hover, filled
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
use glowfly_core::settings::DiffLayout;
use std::time::Instant;

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

/// Toggles lines from the gutter and clicks their text, which leaves them be, checking each
/// click and the selection; snapshots a gutter hovered beside a selected line as `hover_name`
/// and three selected lines as `selected_name`.
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
    assert_eq!(click(&mut h, (at.text)(FIXED)), [], "a click on the text picks no line");
    assert_eq!(click(&mut h, (at.text)(NEW)), [], "not even a selected one's");
    assert_eq!(selection(&h), [NEW], "and leaves the selection as it was");
    click(&mut h, (at.gutter)(NEW));
    assert_eq!(click(&mut h, (at.gutter)(FIXED)), [(FIXED, LineClick::Toggle)]);
    assert_eq!(selection(&h), [FIXED]);
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
    // The hover's text is the dark test's; the selection's look is the light golden's.
    gutter_test(DiffLayout::Inline, false, None, "diff_lines_gutter_selected_light");
}

#[test]
fn diff_gutter_toggles_lines_split() {
    gutter_test(DiffLayout::Split, true, None, "diff_lines_gutter_split");
}

/// The gutter's hover text on a commit diff, where lines are picked for Revert lines.
const REVERT_TIP: &str = "Select line for Revert lines";

/// Hovers `at` long enough for a tooltip to show (the app's 400 ms delay).
fn hover_long(h: &mut H, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    for _ in 0..4 {
        h.run();
    }
}

#[test]
fn diff_commit_gutter_hover_says_how_to_select() {
    use super::super::snapshot_tests_revert::with_line_diff;
    use glowfly_core::settings::ThemeChoice;
    // ` app`, `-fix`, `+fix the build`, `+on Windows`: line i is i + 1 rows below the header.
    let (st, dir) = with_line_diff("gutter-tip", ThemeChoice::Dark, DiffLayout::Inline, &[]);
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("Revert hunk").bounding_box().expect("the button has bounds");
    let y = (b.y0 + b.y1) as f32 / 2.0 + 3.0 * row_h;
    // Over the code: no hover text (and no check box or bar).
    hover_long(&mut h, pos2(261.0 + 200.0, y));
    assert!(h.query_by_label_contains(REVERT_TIP).is_none(), "the code says nothing about lines");
    // Over the gutter of `+fix the build`: the hollow check box, the faint bar and the hover text.
    hover_long(&mut h, pos2(261.0 + 100.0, y));
    assert!(h.query_by_label_contains(REVERT_TIP).is_some(), "the gutter says how to pick lines");
    h.check_snapshot("diff_commit_gutter_hover");
    drop(h);
    remove_fixture(&dir);

    // A working-tree diff's gutter picks lines for staging, so the Revert lines text isn't shown.
    let (st, dir) = with_diff(DiffLayout::Inline, "gutter-tip-wip");
    let mut h = harness(st, true);
    h.run();
    let at = spots(&h, DiffLayout::Inline);
    hover_long(&mut h, (at.gutter)(NEW));
    assert!(h.query_by_label_contains(REVERT_TIP).is_none(), "no Revert lines text on a working-tree diff");
    drop(h);
    remove_fixture(&dir);
}

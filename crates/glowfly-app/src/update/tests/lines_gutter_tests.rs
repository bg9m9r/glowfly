//! Headless tests for picking lines from the diff's gutter: a click there toggles the line
//! (`LineClick::Toggle`, as the view sends it) and leaves the others selected, while a click on
//! the text (`Only`) starts over. The selection stays within one hunk, since a line operation
//! applies to one hunk.

use super::lines_tests::{click, run, selected, unstaged, with_diff};
use crate::msg::{DiffMsg, Msg};
use crate::state::lines::LineClick::{Only, Range, Toggle};

#[test]
fn gutter_clicks_add_and_remove_lines() {
    let mut st = with_diff(unstaged());
    // From nothing, a gutter click selects the line.
    click(&mut st, 0, 1, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1])));
    // Each further one adds its line and keeps the rest.
    click(&mut st, 0, 4, Toggle);
    click(&mut st, 0, 5, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 4, 5])));
    // The hunk header's "Stage N lines" counts them.
    assert_eq!(st.view.diff_lines.count_in(&unstaged(), 0), 3);
    // A second click on a selected line's gutter takes it out again.
    click(&mut st, 0, 4, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 5])));
    assert_eq!(st.view.diff_lines.count_in(&unstaged(), 0), 2);
    // A context line's gutter is not a toggle.
    click(&mut st, 0, 3, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 5])));
    // Taking out the last line leaves nothing selected.
    click(&mut st, 0, 1, Toggle);
    click(&mut st, 0, 5, Toggle);
    assert_eq!(selected(&st), None);
}

#[test]
fn text_click_replaces_and_shift_runs_from_the_last_toggle() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Toggle);
    click(&mut st, 0, 5, Toggle);
    // A click on a line's text selects only that line.
    click(&mut st, 0, 2, Only);
    assert_eq!(selected(&st), Some((0, vec![2])));
    // Shift-click (in the gutter or on the text) runs from the line toggled last.
    click(&mut st, 0, 1, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 2])));
    click(&mut st, 0, 5, Range);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4, 5])));
    // Esc (ClearLineSelection) drops it.
    run(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    assert_eq!(selected(&st), None);
}

#[test]
fn a_gutter_click_in_another_hunk_starts_a_selection_there() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Toggle);
    click(&mut st, 0, 2, Toggle);
    // Line operations apply to one hunk, so another hunk's line replaces the selection.
    click(&mut st, 1, 1, Toggle);
    assert_eq!(selected(&st), Some((1, vec![1])));
    assert_eq!(st.view.diff_lines.count_in(&unstaged(), 0), 0);
    click(&mut st, 1, 2, Toggle);
    assert_eq!(selected(&st), Some((1, vec![1, 2])));
}

#[test]
fn a_drag_down_the_gutter_adds_to_the_selection() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Toggle);
    run(&mut st, Msg::Diff(DiffMsg::StartLineDrag { hunk: 0, line: 4, how: Toggle }));
    run(&mut st, Msg::Diff(DiffMsg::DragDiffLine { hunk: 0, line: 5 }));
    run(&mut st, Msg::Diff(DiffMsg::EndLineDrag));
    assert_eq!(selected(&st), Some((0, vec![1, 4, 5])));
}

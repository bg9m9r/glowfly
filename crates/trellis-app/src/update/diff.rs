//! Diff viewer layout, view mode (hunks or the whole file) and navigation.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use trellis_core::diff_rows::{header_rows, hunk_at, step_change};
use trellis_core::repo::FileDiff;
use trellis_core::settings::{DiffLayout, DiffMode, Settings};
use trellis_core::whole_file::{change_of_hunk, change_rows, hunk_of_change, shown};

/// The row in layout `to` showing the change (hunk, or change block in the whole-file view)
/// that row `top` of layout `from` is in, so the change at the top of the viewport stays there.
pub(super) fn same_change_row(d: &FileDiff, mode: DiffMode, from: DiffLayout, to: DiffLayout, top: usize) -> usize {
    if from == to {
        return top;
    }
    let i = hunk_at(&change_rows(d, mode, from), top);
    change_rows(d, mode, to).get(i).copied().unwrap_or(0)
}

/// The normal hunk whose header or change block is at row `top`, for `mode` and `layout`.
pub(super) fn hunk_at_top(d: &FileDiff, mode: DiffMode, layout: DiffLayout, top: usize) -> Option<usize> {
    hunk_of_change(d, mode, hunk_at(&change_rows(d, mode, layout), top))
}

/// The row a freshly opened diff starts at: hunk `hunk`'s header or change block when given;
/// otherwise the top in the hunk view and the first change block in the whole-file view.
pub(super) fn opening_row(d: &FileDiff, s: &Settings, hunk: Option<usize>) -> usize {
    let mode = s.diff_mode;
    let change = hunk.and_then(|h| change_of_hunk(d, mode, h)).or(shown(d, mode).map(|_| 0));
    change.and_then(|i| change_rows(d, mode, s.diff_layout).get(i).copied()).unwrap_or(0)
}

pub(super) fn handle(st: &mut AppState, msg: Msg, fx: &mut Vec<Effect>) {
    match msg {
        Msg::SetDiffLayout(layout) => {
            if layout == st.settings.diff_layout {
                return;
            }
            if let Some((_, d)) = &st.diff {
                let row = same_change_row(d, st.settings.diff_mode, st.settings.diff_layout, layout, st.view.diff_top_row);
                st.view.diff_scroll_to_row = Some(row);
                st.view.diff_top_row = row;
            }
            st.settings.diff_layout = layout;
            fx.push(super::prefs::save_settings(st));
        }
        Msg::SetDiffMode(mode) => set_mode(st, mode, fx),
        Msg::SetIgnoreWhitespace(on) => {
            if on == st.settings.diff_ignore_whitespace {
                return;
            }
            st.settings.diff_ignore_whitespace = on;
            st.view.diff_lines.clear();
            fx.push(super::prefs::save_settings(st));
            // Hunk rows change, so the diff reopens at the top; hunk operations wait for it.
            super::repo::reopen_diff(st, fx);
        }
        Msg::SelectDiffLine { hunk, line, how } => {
            if st.line_ops_blocked().is_some() {
                return;
            }
            if let Some((target, d)) = &st.diff {
                st.view.diff_lines.click(target, d, hunk, line, how);
            }
        }
        Msg::ClearLineSelection => st.view.diff_lines.clear(),
        Msg::JumpToChange(step) => {
            let Some((_, d)) = &st.diff else { return };
            let rows = change_rows(d, st.settings.diff_mode, st.settings.diff_layout);
            if let Some(row) = step_change(&rows, st.view.diff_top_row, step) {
                st.view.diff_scroll_to_row = Some(row);
                // Recorded now so repeated presses before the next frame keep stepping.
                st.view.diff_top_row = row;
            }
        }
        _ => {}
    }
}

/// Switches between hunks and the whole file, keeping the hunk at the top in view.
fn set_mode(st: &mut AppState, mode: DiffMode, fx: &mut Vec<Effect>) {
    let (old, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    if mode == old {
        return;
    }
    let h = st.diff.as_ref().and_then(|(_, d)| hunk_at_top(d, old, layout, st.view.diff_top_row));
    st.settings.diff_mode = mode;
    st.view.diff_lines.clear();
    fx.push(super::prefs::save_settings(st));
    match mode {
        DiffMode::WholeFile => {
            // The whole file comes with a reload; it opens on the same hunk's block, and hunk
            // operations wait for it.
            st.view.diff_open_hunk = h;
            super::repo::reopen_diff(st, fx);
        }
        DiffMode::Hunks => {
            // The normal hunks are already loaded, and still current for hunk operations.
            let Some((_, d)) = st.diff.as_mut() else { return };
            d.whole = None;
            let row = h.and_then(|h| header_rows(d, layout).get(h).copied()).unwrap_or(0);
            st.view.diff_scroll_to_row = Some(row);
            st.view.diff_top_row = row;
        }
    }
}

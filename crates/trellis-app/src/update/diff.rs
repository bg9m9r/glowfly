//! Diff viewer layout and navigation.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use trellis_core::diff_rows::{header_rows, hunk_at, step_change};
use trellis_core::repo::FileDiff;
use trellis_core::settings::DiffLayout;

/// The row in layout `to` showing the hunk that row `top` of layout `from` is in, so the hunk
/// at the top of the viewport stays at the top.
pub(super) fn same_hunk_row(d: &FileDiff, from: DiffLayout, to: DiffLayout, top: usize) -> usize {
    if from == to {
        return top;
    }
    let h = hunk_at(&header_rows(d, from), top);
    header_rows(d, to).get(h).copied().unwrap_or(0)
}

pub(super) fn handle(st: &mut AppState, msg: Msg, fx: &mut Vec<Effect>) {
    match msg {
        Msg::SetDiffLayout(layout) => {
            if layout == st.settings.diff_layout {
                return;
            }
            if let Some((_, d)) = &st.diff {
                let row = same_hunk_row(d, st.settings.diff_layout, layout, st.view.diff_top_row);
                st.view.diff_scroll_to_row = Some(row);
                st.view.diff_top_row = row;
            }
            st.settings.diff_layout = layout;
            fx.push(super::prefs::save_settings(st));
        }
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
            if st.hunk_ops_blocked().is_some() {
                return;
            }
            if let Some((target, d)) = &st.diff {
                st.view.diff_lines.click(target, d, hunk, line, how);
            }
        }
        Msg::ClearLineSelection => st.view.diff_lines.clear(),
        Msg::JumpToChange(step) => {
            let Some((_, d)) = &st.diff else { return };
            let headers = header_rows(d, st.settings.diff_layout);
            if let Some(row) = step_change(&headers, st.view.diff_top_row, step) {
                st.view.diff_scroll_to_row = Some(row);
                // Recorded now so repeated presses before the next frame keep stepping.
                st.view.diff_top_row = row;
            }
        }
        _ => {}
    }
}

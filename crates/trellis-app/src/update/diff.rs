//! Diff viewer layout, view mode (hunks or the whole file) and navigation.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::diff_nav::{self, Landing, Step};
use trellis_core::diff_rows::{header_rows, hunk_at};
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::{DiffLayout, DiffMode, Settings};
use trellis_core::whole_file::{Expanded, change_of_hunk, change_rows, folds, hunk_of_change, shown};

/// The row in layout `to` showing the change (hunk, or change block in the whole-file view)
/// that row `top` of layout `from` is in, so the change at the top of the viewport stays there.
/// Both layouts count the whole-file rows with the folds in `ex` open.
pub(super) fn same_change_row(d: &FileDiff, mode: DiffMode, from: DiffLayout, to: DiffLayout, top: usize, ex: &Expanded) -> usize {
    if from == to {
        return top;
    }
    let i = hunk_at(&change_rows(d, mode, from, ex), top);
    change_rows(d, mode, to, ex).get(i).copied().unwrap_or(0)
}

/// The normal hunk whose header or change block is at row `top`, for `mode` and `layout`, with
/// the folds in `ex` open.
pub(super) fn hunk_at_top(d: &FileDiff, mode: DiffMode, layout: DiffLayout, top: usize, ex: &Expanded) -> Option<usize> {
    hunk_of_change(d, mode, hunk_at(&change_rows(d, mode, layout, ex), top))
}

/// The row a freshly opened diff starts at: hunk `hunk`'s header or change block when given;
/// otherwise the top in the hunk view and the first change block in the whole-file view.
/// Rows are counted with the folds in `ex` open.
pub(super) fn opening_row(d: &FileDiff, s: &Settings, hunk: Option<usize>, ex: &Expanded) -> usize {
    let mode = s.diff_mode;
    let change = hunk.and_then(|h| change_of_hunk(d, mode, h)).or(shown(d, mode).map(|_| 0));
    change.and_then(|i| change_rows(d, mode, s.diff_layout, ex).get(i).copied()).unwrap_or(0)
}

/// The row a file opened by next/previous change lands on: its first or last change row,
/// counted with the folds in `ex` open.
pub(super) fn landing_row(d: &FileDiff, s: &Settings, l: Landing, ex: &Expanded) -> usize {
    let rows = change_rows(d, s.diff_mode, s.diff_layout, ex);
    let row = match l {
        Landing::First => rows.first(),
        Landing::Last => rows.last(),
    };
    row.copied().unwrap_or(0)
}

/// Opens `target`, reached by next/previous change, to land on its first or last change.
fn open_landing(st: &mut AppState, target: DiffTarget, landing: Landing, fx: &mut Vec<Effect>) {
    super::repo::open_diff(st, target.clone(), fx);
    // After `open_diff`, which clears it.
    st.view.diff_landing = Some((target, landing));
}

/// A file reached by stepping that has nothing to land on (binary, empty, or only whitespace
/// while it is hidden) is passed over when another file follows in the same direction: that one
/// opens instead. Returns whether it did.
pub(super) fn skip_past(st: &mut AppState, target: &DiffTarget, d: &FileDiff, l: Landing, fx: &mut Vec<Effect>) -> bool {
    if !diff_nav::change_rows_of(d, &st.settings, &st.view.diff_expanded).is_empty() {
        return false;
    }
    match diff_nav::next_file(st, target, l.dir()) {
        Some(next) => {
            open_landing(st, next, l, fx);
            true
        }
        None => false,
    }
}

pub(super) fn handle(st: &mut AppState, msg: Msg, fx: &mut Vec<Effect>) {
    match msg {
        Msg::SetDiffLayout(layout) => {
            if layout == st.settings.diff_layout {
                return;
            }
            if let Some((_, d)) = &st.diff {
                let (mode, from, top) = (st.settings.diff_mode, st.settings.diff_layout, st.view.diff_top_row);
                let row = same_change_row(d, mode, from, layout, top, &st.view.diff_expanded);
                st.view.diff_scroll_to_row = Some(row);
                st.view.diff_top_row = row;
                st.view.diff_cursor = None; // the rows are numbered differently now
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
            if st.view.diff_landing.is_some() {
                return; // a stepped-to file is loading: key repeat waits for it
            }
            match diff_nav::step(st, step) {
                Step::Row(row) => {
                    st.view.diff_scroll_to_row = Some(row);
                    // Recorded now so repeated presses before the next frame keep stepping.
                    st.view.diff_top_row = row;
                    st.view.diff_cursor = Some(row);
                }
                Step::File(target, landing) => open_landing(st, target, landing, fx),
                Step::End => {}
            }
        }
        Msg::ExpandUnchanged(key) => expand(st, key),
        _ => {}
    }
}

/// Switches between hunks and the whole file, keeping the hunk at the top in view.
fn set_mode(st: &mut AppState, mode: DiffMode, fx: &mut Vec<Effect>) {
    let (old, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    if mode == old {
        return;
    }
    let h = st.diff.as_ref().and_then(|(_, d)| hunk_at_top(d, old, layout, st.view.diff_top_row, &st.view.diff_expanded));
    st.settings.diff_mode = mode;
    st.view.diff_lines.clear();
    fx.push(super::prefs::save_settings(st));
    match mode {
        DiffMode::WholeFile => {
            // The whole file comes with a reload; it opens on the same hunk's block, and hunk
            // operations wait for it.
            st.view.diff_open_hunk = h;
            st.view.diff_cursor = None;
            super::repo::reopen_diff(st, fx);
        }
        DiffMode::Hunks => {
            // The normal hunks are already loaded, and still current for hunk operations.
            let Some((_, d)) = st.diff.as_mut() else { return };
            d.whole = None;
            let row = h.and_then(|h| header_rows(d, layout).get(h).copied()).unwrap_or(0);
            st.view.diff_scroll_to_row = Some(row);
            st.view.diff_top_row = row;
            st.view.diff_cursor = None;
        }
    }
}

/// Opens the whole-file fold `key` in place. Rows above it keep their indexes, so nothing
/// scrolls. Ignored when `key` isn't a fold of the whole file on screen (a click that raced a
/// reload or close) or is already open.
fn expand(st: &mut AppState, key: u32) {
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let Some((_, d)) = &st.diff else { return };
    let Some(w) = shown(d, mode) else { return };
    if !folds(w).iter().any(|f| f.key == key) {
        return;
    }
    let before = change_rows(d, mode, layout, &st.view.diff_expanded);
    if !st.view.diff_expanded.insert(key) {
        return;
    }
    let after = change_rows(d, mode, layout, &st.view.diff_expanded);
    // The change last stepped to keeps its place among the change rows.
    st.view.diff_cursor = st.view.diff_cursor.and_then(|c| before.iter().position(|&r| r == c)).and_then(|i| after.get(i).copied());
}

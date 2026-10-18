//! The keyboard's way to the unchanged runs: Show (open the first closed run at or below the top
//! row) and Fold back (close the opened run at the top). Whole-file folds in the whole-file
//! view, the gaps between hunks in the hunk view; each acts like a click on that run's control.

use super::diff::refold;
use super::diff_gaps;
use crate::effect::Effect;
use crate::state::AppState;
use trellis_core::diff_rows::{DiffRow, SplitRow, filled, header_rows_filled, inline_rows_filled, split_rows_filled};
use trellis_core::settings::DiffLayout;
use trellis_core::whole_file::{collapsed, line_at_row, opened, row_of_line, shown};

/// The rows from the top row down that fit in the viewport (at least one).
fn screen(st: &AppState) -> std::ops::Range<usize> {
    let top = st.view.diff_top_row;
    top..top + st.view.diff_visible_rows.max(1)
}

/// The row of line `line` of the gap before hunk `hunk` in the hunk view's rows, once filled in.
fn gap_line_row(st: &AppState, hunk: usize, line: usize) -> Option<usize> {
    let (_, d) = st.diff.as_ref()?;
    let ex = &st.view.diff_expanded;
    match st.settings.diff_layout {
        DiffLayout::Inline => inline_rows_filled(d, ex).iter().position(|r| *r == DiffRow::GapLine { hunk, line }),
        DiffLayout::Split => split_rows_filled(d, ex).iter().position(|r| *r == SplitRow::GapLine { hunk, line }),
    }
}

/// Opens the first closed run whose row is at or after the top row. When that row is below the
/// viewport, the view scrolls so the opened lines start at the top; otherwise nothing scrolls,
/// as with a click.
pub(super) fn open_at_top(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (mode, layout, below) = (st.settings.diff_mode, st.settings.diff_layout, screen(st));
    let Some((_, d)) = st.diff.as_ref().map(|(t, d)| (t, d.clone())) else { return };
    let ex = &st.view.diff_expanded;
    if let Some(w) = shown(&d, mode) {
        let first = collapsed(w, ex).into_iter().map(|f| (f, row_of_line(w, layout, ex, f.start))).find(|&(_, row)| row >= below.start);
        let Some((f, row)) = first else { return };
        refold(st, |ex, _| ex.insert(f.key));
        if row >= below.end {
            let to = row_of_line(w, layout, &st.view.diff_expanded, f.start);
            (st.view.diff_top_row, st.view.diff_scroll_to_row) = (to, Some(to));
        }
        return;
    }
    let Some((_, all)) = diff_gaps::shown_gaps(st) else { return };
    let open: Vec<u32> = filled(&d, ex).iter().map(|(g, _)| g.key).collect();
    let headers = header_rows_filled(&d, layout, ex);
    let first = all.into_iter().filter(|g| !open.contains(&g.key)).map(|g| (g, headers[g.hunk])).find(|&(_, row)| row >= below.start);
    let Some((g, row)) = first else { return };
    diff_gaps::open(st, g.key, fx);
    if row >= below.end {
        // Its first line once the text is there; until then the header after it.
        let to = gap_line_row(st, g.hunk, 0).unwrap_or(row);
        (st.view.diff_top_row, st.view.diff_scroll_to_row) = (to, Some(to));
    }
}

/// Closes the opened run that holds the top row, else the first opened run whose first line is
/// on screen, keeping the same line at the top as a click on its fold-back control does.
pub(super) fn close_at_top(st: &mut AppState) {
    let (mode, layout, screen) = (st.settings.diff_mode, st.settings.diff_layout, screen(st));
    let Some((_, d)) = st.diff.as_ref().map(|(t, d)| (t, d.clone())) else { return };
    let ex = &st.view.diff_expanded;
    if let Some(w) = shown(&d, mode) {
        let open = opened(w, ex);
        let at = line_at_row(w, layout, ex, screen.start);
        let holding = open.iter().find(|f| at.is_some_and(|l| f.start <= l && l < f.end));
        let on_screen = || open.iter().find(|f| screen.contains(&row_of_line(w, layout, ex, f.start)));
        let Some(key) = holding.or_else(on_screen).map(|f| f.key) else { return };
        refold(st, |ex, _| ex.remove(key));
        return;
    }
    if diff_gaps::shown_gaps(st).is_none() {
        return;
    }
    let open: Vec<_> = filled(&d, ex).into_iter().map(|(g, _)| g).collect();
    let top_hunk = match layout {
        DiffLayout::Inline => match inline_rows_filled(&d, ex).get(screen.start) {
            Some(DiffRow::GapLine { hunk, .. }) => Some(*hunk),
            _ => None,
        },
        DiffLayout::Split => match split_rows_filled(&d, ex).get(screen.start) {
            Some(SplitRow::GapLine { hunk, .. }) => Some(*hunk),
            _ => None,
        },
    };
    let holding = open.iter().find(|g| Some(g.hunk) == top_hunk);
    let on_screen = || open.iter().find(|g| gap_line_row(st, g.hunk, 0).is_some_and(|r| screen.contains(&r)));
    let Some(key) = holding.or_else(on_screen).map(|g| g.key) else { return };
    diff_gaps::close(st, key);
}

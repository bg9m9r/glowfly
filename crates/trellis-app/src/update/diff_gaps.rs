//! The unchanged lines between hunks in the hunk view: opening and closing a run (by the key it
//! shares with the whole-file view's fold of the same lines), reading the runs' text once per
//! diff (`Effect::LoadDiffGaps`), and keeping the row at the top in place as rows come and go.

use crate::effect::Effect;
use crate::state::AppState;
use std::sync::Arc;
use trellis_core::diff_rows::{DiffRow, GapSpan, GapText, SplitRow, gaps, inline_rows_filled, split_rows_filled};
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::DiffLayout;
use trellis_core::whole_file::{Expanded, change_rows, shown};

/// The gaps of the diff on screen when the hunk view shows it and they can be filled in: never
/// with whitespace hidden, as the unchanged runs can hold hidden whitespace changes. `None`
/// otherwise.
pub(super) fn shown_gaps(st: &AppState) -> Option<(Arc<FileDiff>, Vec<GapSpan>)> {
    let (_, d) = st.diff.as_ref()?;
    if st.settings.diff_ignore_whitespace || shown(d, st.settings.diff_mode).is_some() {
        return None;
    }
    Some((d.clone(), gaps(d)))
}

/// Whether the text of `g` has been read.
fn has_text(ex: &Expanded, g: &GapSpan) -> bool {
    ex.gap_text().is_some_and(|t| t.lines_for(g).is_some())
}

/// Opens the gap `key` of the diff on screen, reading the gaps' text unless it is there or on
/// its way (another opened gap is still waiting for it).
pub(super) fn open(st: &mut AppState, key: u32, fx: &mut Vec<Effect>) {
    let Some((_, all)) = shown_gaps(st) else { return };
    let Some(g) = all.iter().find(|g| g.key == key) else { return };
    let ex = &st.view.diff_expanded;
    // An opened gap (this one included, from an earlier click) still waits for the text.
    let waiting = all.iter().any(|o| ex.contains(o.key) && !has_text(ex, o));
    let need = !has_text(ex, g) && !waiting;
    if !reanchor(st, |ex| ex.insert(key)) && !need {
        return;
    }
    if need {
        request(st, fx);
    }
}

/// Closes the gap `key` of the diff on screen.
pub(super) fn close(st: &mut AppState, key: u32) {
    let Some((_, all)) = shown_gaps(st) else { return };
    if all.iter().any(|g| g.key == key) {
        reanchor(st, |ex| ex.remove(key));
    }
}

/// The gaps' text read for the diff shown at `generation`: kept when that diff is still on
/// screen. When it can't be read, the gaps waiting for it close and the log says why.
pub(super) fn loaded(st: &mut AppState, target: DiffTarget, generation: u64, result: Result<Arc<GapText>, String>) {
    if generation != st.diff_shown_gen || !st.diff.as_ref().is_some_and(|(t, _)| *t == target) {
        return;
    }
    match result {
        // Outside the hunk view the rows don't change.
        Ok(text) if shown_gaps(st).is_none() => st.view.diff_expanded.set_gap_text(text),
        Ok(text) => {
            reanchor(st, |ex| {
                ex.set_gap_text(text);
                true
            });
        }
        Err(e) => {
            let all = shown_gaps(st).map(|(_, all)| all).unwrap_or_default();
            let ex = &mut st.view.diff_expanded;
            let waiting: Vec<u32> = all.iter().filter(|g| ex.contains(g.key) && !has_text(ex, g)).map(|g| g.key).collect();
            for key in waiting {
                ex.remove(key);
            }
            st.log(false, format!("These unchanged lines can't be shown: {e}"));
        }
    }
}

/// A diff was shown (opened or reloaded): reads the gaps' text again when an opened gap of the
/// hunk view is among its gaps. The text read before stays until the new one lands; gaps that
/// moved show closed meanwhile.
pub(super) fn diff_shown(st: &AppState, fx: &mut Vec<Effect>) {
    let Some((_, all)) = shown_gaps(st) else { return };
    if all.iter().any(|g| st.view.diff_expanded.contains(g.key)) {
        request(st, fx);
    }
}

fn request(st: &AppState, fx: &mut Vec<Effect>) {
    if let (Some(workdir), Some((target, diff))) = (st.workdir(), st.diff.as_ref()) {
        fx.push(Effect::LoadDiffGaps { workdir, target: target.clone(), generation: st.diff_shown_gen, diff: diff.clone() });
    }
}

/// The hunk view's rows in either layout, so the top row can be found again.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Inline(DiffRow),
    Split(SplitRow),
}

impl Row {
    /// The header after a filled-in gap line, where the view goes when the gap closes.
    fn header_after(self) -> Option<Row> {
        match self {
            Row::Inline(DiffRow::GapLine { hunk, .. }) => Some(Row::Inline(DiffRow::Header(hunk))),
            Row::Split(SplitRow::GapLine { hunk, .. }) => Some(Row::Split(SplitRow::Header(hunk))),
            _ => None,
        }
    }
}

fn rows(d: &FileDiff, layout: DiffLayout, ex: &Expanded) -> Vec<Row> {
    match layout {
        DiffLayout::Inline => inline_rows_filled(d, ex).into_iter().map(Row::Inline).collect(),
        DiffLayout::Split => split_rows_filled(d, ex).into_iter().map(Row::Split).collect(),
    }
}

/// Applies `change` to the opened runs (it says whether it changed anything), keeping the row at
/// the top of the viewport there and the change last stepped to in its place among the change
/// rows. Returns what `change` returned.
pub(super) fn reanchor(st: &mut AppState, change: impl FnOnce(&mut Expanded) -> bool) -> bool {
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let Some((_, d)) = st.diff.as_ref().map(|(t, d)| (t, d.clone())) else { return false };
    let ex = &mut st.view.diff_expanded;
    let (before, changes) = (rows(&d, layout, ex), change_rows(&d, mode, layout, ex));
    if !change(ex) {
        return false;
    }
    let (after, now) = (rows(&d, layout, ex), change_rows(&d, mode, layout, ex));
    st.view.diff_cursor = st.view.diff_cursor.and_then(|c| changes.iter().position(|&r| r == c)).and_then(|i| now.get(i).copied());
    let top = st.view.diff_top_row;
    let moved = before
        .get(top)
        .and_then(|&r| after.iter().position(|&x| x == r).or_else(|| r.header_after().and_then(|h| after.iter().position(|&x| x == h))));
    if let Some(row) = moved.filter(|&row| row != top) {
        st.view.diff_top_row = row;
        st.view.diff_scroll_to_row = Some(row);
    }
    true
}

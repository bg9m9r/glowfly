//! The unchanged runs of the hunk view (above the first hunk, between hunks, below the last):
//! opening and closing them one at a time or all at once (by the key each shares with the
//! whole-file view's fold of the same lines), reading their text once per diff
//! (`DiffEffect::Gaps`), and keeping the row at the top in place as rows come and go.

use crate::effect::DiffEffect;
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::diff_runs::{hunk_gaps, reading};
use crate::state::error_words::typed;
use glowfly_core::diff_rows::{DiffRow, GapSpan, GapText, SplitRow, gaps, inline_rows_filled, split_rows_filled};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::DiffLayout;
use glowfly_core::whole_file::{Expanded, change_rows};
use std::sync::Arc;

/// Whether the text of `g` has been read.
fn has_text(ex: &Expanded, g: &GapSpan) -> bool {
    ex.gap_text().is_some_and(|t| t.lines_for(g).is_some())
}

/// Opens the run `key` of the diff on screen, reading the runs' text unless it is there or on
/// its way.
pub(super) fn open(st: &mut AppState, key: u32, fx: &mut Vec<Effect>) {
    let Some((_, all)) = hunk_gaps(st) else { return };
    let Some(g) = all.iter().find(|g| g.key == key) else { return };
    let need = !has_text(&st.session.tab_view.diff_expanded, g) && !reading(st);
    if !reanchor(st, |ex| ex.insert(key)) && !need {
        return;
    }
    if need {
        request(st, fx);
    }
}

/// Opens every run of the diff on screen with one read of the text (none when it is there or
/// on its way).
pub(super) fn open_all(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some((_, all)) = hunk_gaps(st) else { return };
    reanchor(st, |ex| all.iter().fold(false, |changed, g| ex.insert(g.key) | changed));
    let ex = &st.session.tab_view.diff_expanded;
    if all.iter().any(|g| !has_text(ex, g)) && !reading(st) {
        request(st, fx);
    }
}

/// Closes the run `key` of the diff on screen.
pub(super) fn close(st: &mut AppState, key: u32) {
    let Some((_, all)) = hunk_gaps(st) else { return };
    if all.iter().any(|g| g.key == key) {
        reanchor(st, |ex| ex.remove(key));
    }
}

/// Closes every run of the diff on screen, those still waiting for their text included, and
/// forgets keys of runs a reload has since removed, so none opens again by chance. The text
/// stays.
pub(super) fn close_all(st: &mut AppState) {
    if hunk_gaps(st).is_none() {
        return;
    }
    reanchor(st, |ex| {
        let open: Vec<u32> = ex.keys().collect();
        open.into_iter().fold(false, |changed, key| ex.remove(key) | changed)
    });
}

/// The runs' text read for the diff shown at `generation`: kept when that diff is still on
/// screen (and shows whitespace changes, else the text would count rows the view doesn't draw).
/// When it can't be read, the runs waiting for it close and the log says why.
pub(super) fn loaded(st: &mut AppState, target: DiffTarget, generation: u64, result: Result<Arc<GapText>, ReadFailed>) {
    if generation != st.session.diff.shown_gen || !st.session.diff.view.as_ref().is_some_and(|(t, _)| *t == target) {
        return;
    }
    st.session.tab_view.diff_gaps_reading = None;
    match result {
        Ok(_) if st.settings.diff_ignore_whitespace => {}
        // Outside the hunk view the rows don't change.
        Ok(text) if hunk_gaps(st).is_none() => st.session.tab_view.diff_expanded.set_gap_text(text),
        Ok(text) => {
            reanchor(st, |ex| {
                ex.set_gap_text(text);
                true
            });
        }
        Err(e) => {
            let all = hunk_gaps(st).map(|(_, all)| all).unwrap_or_default();
            let ex = &mut st.session.tab_view.diff_expanded;
            let waiting: Vec<u32> = all.iter().filter(|g| ex.contains(g.key) && !has_text(ex, g)).map(|g| g.key).collect();
            for key in waiting {
                ex.remove(key);
            }
            let t = st.settings.lang();
            st.log(false, t.trf("diff.gaps.failed", &[("why", &typed::read(t, &e))]));
        }
    }
}

/// A diff was shown (opened or reloaded): reads the runs' text again when an opened run of the
/// hunk view is among its runs (unless a read is on its way). The text read before stays until
/// the new one lands; runs that moved show closed meanwhile.
pub(super) fn diff_shown(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some((_, all)) = hunk_gaps(st) else { return };
    if all.iter().any(|g| st.session.tab_view.diff_expanded.contains(g.key)) && !reading(st) {
        request(st, fx);
    }
}

/// Asks for the runs' text of the diff on screen, and notes it is on its way.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    let generation = st.session.diff.shown_gen;
    if let (Some(workdir), Some((target, diff))) = (st.workdir(), st.session.diff.view.as_ref()) {
        fx.push(Effect::Diff(DiffEffect::Gaps { workdir, target: target.clone(), generation, diff: diff.clone() }));
        st.session.tab_view.diff_gaps_reading = Some(generation);
    }
}

/// The hunk view's rows in either layout, so the top row can be found again.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Inline(DiffRow),
    Split(SplitRow),
}

impl Row {
    /// The control after a filled-in run's line, where the view goes when the run closes: the
    /// header of the hunk after it, or for the run below the last hunk its own row.
    fn control_after(self, d: &FileDiff) -> Option<Row> {
        let below = |hunk: usize| gaps(d).into_iter().find(|g| g.hunk == hunk && hunk == d.hunks.len());
        match self {
            Row::Inline(DiffRow::GapLine { hunk, .. }) => Some(Row::Inline(below(hunk).map_or(DiffRow::Header(hunk), DiffRow::GapBelow))),
            Row::Split(SplitRow::GapLine { hunk, .. }) => Some(Row::Split(below(hunk).map_or(SplitRow::Header(hunk), SplitRow::GapBelow))),
            _ => None,
        }
    }

    /// The first line of the run whose header this is: it takes the header's place when the run
    /// opens.
    fn first_line_of_run(self) -> Option<Row> {
        match self {
            Row::Inline(DiffRow::Header(hunk)) => Some(Row::Inline(DiffRow::GapLine { hunk, line: 0 })),
            Row::Split(SplitRow::Header(hunk)) => Some(Row::Split(SplitRow::GapLine { hunk, line: 0 })),
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
/// the top of the viewport there (a header whose own run opens gives its place to the run's first
/// line) and the change last stepped to in its place among the change rows. Returns what
/// `change` returned.
pub(super) fn reanchor(st: &mut AppState, change: impl FnOnce(&mut Expanded) -> bool) -> bool {
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let Some((_, d)) = st.session.diff.view.as_ref().map(|(t, d)| (t, d.clone())) else { return false };
    let ex = &mut st.session.tab_view.diff_expanded;
    let (before, changes) = (rows(&d, layout, ex), change_rows(&d, mode, layout, ex));
    if !change(ex) {
        return false;
    }
    let (after, now) = (rows(&d, layout, ex), change_rows(&d, mode, layout, ex));
    st.cues.diff_cursor = st.cues.diff_cursor.and_then(|c| changes.iter().position(|&r| r == c)).and_then(|i| now.get(i).copied());
    let top = st.view.tab.diff_top_row;
    let opened = before.get(top).and_then(|r| r.first_line_of_run()).filter(|first| !before.contains(first));
    let moved = opened.and_then(|first| after.iter().position(|&x| x == first)).or_else(|| {
        let r = *before.get(top)?;
        after.iter().position(|&x| x == r).or_else(|| r.control_after(&d).and_then(|c| after.iter().position(|&x| x == c)))
    });
    if let Some(row) = moved.filter(|&row| row != top) {
        st.view.tab.diff_top_row = row;
        st.view.diff_scroll_to_row = Some(row);
    }
    true
}

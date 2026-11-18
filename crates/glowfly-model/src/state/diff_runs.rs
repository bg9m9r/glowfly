//! The unchanged runs of the open diff as the view and the menus see them: the hunk view's runs
//! (above the first hunk, between hunks, below the last) with how many are open, the whole-file
//! view's folds the same way, and whether the text of the hunk view's runs is being read.

use super::AppState;
use super::diff_header::FoldCounts;
use glowfly_core::diff_rows::{GapSpan, gaps};
use glowfly_core::repo::FileDiff;
use glowfly_core::whole_file::{folds, shown};
use std::sync::Arc;

#[cfg(test)]
#[path = "diff_runs_tests.rs"]
mod tests;

/// The runs of the diff on screen when the hunk view shows it and they can be filled in: never
/// with whitespace hidden, as the unchanged runs can hold hidden whitespace changes. `None`
/// otherwise.
pub fn hunk_gaps(st: &AppState) -> Option<(Arc<FileDiff>, Vec<GapSpan>)> {
    let (_, d) = st.session.diff.view.as_ref()?;
    if st.settings.diff_ignore_whitespace || shown(d, st.settings.diff_mode).is_some() {
        return None;
    }
    Some((d.clone(), gaps(d)))
}

/// Whether the text of the runs is on its way for the diff on screen (`DiffEffect::Gaps`).
pub fn reading(st: &AppState) -> bool {
    st.session.tab_view.diff_gaps_reading == Some(st.session.diff.shown_gen)
}

/// How many runs of `diff` (the diff on screen) are open and closed: the whole-file view's
/// folds there, else the hunk view's runs. Runs whose text is still being read count as open.
/// `None` when there are no runs.
pub fn counts_of(st: &AppState, diff: &FileDiff) -> Option<FoldCounts> {
    let ex = &st.session.tab_view.diff_expanded;
    let counts = if let Some(w) = shown(diff, st.settings.diff_mode) {
        let all = folds(w);
        let open = all.iter().filter(|f| ex.contains(f.key)).count();
        FoldCounts { open, closed: all.len() - open, hunks: false, ws_hidden: false }
    } else {
        let all = gaps(diff);
        if st.settings.diff_ignore_whitespace {
            FoldCounts { open: 0, closed: all.len(), hunks: true, ws_hidden: true }
        } else {
            let open = all.iter().filter(|g| ex.contains(g.key)).count();
            FoldCounts { open, closed: all.len() - open, hunks: true, ws_hidden: false }
        }
    };
    (counts.open + counts.closed > 0).then_some(counts)
}

/// `counts_of` for the diff on screen, when it shows text rows: not a binary file, an LFS
/// summary or an SVG's preview, and with hunks.
pub fn counts(st: &AppState) -> Option<FoldCounts> {
    let (_, diff) = st.session.diff.view.as_ref()?;
    let text_rows = !diff.binary && !diff.hunks.is_empty() && super::lfs_notes::summary(st).is_none();
    if !text_rows || super::diff_preview::svg_switch(st) == Some(true) {
        return None;
    }
    counts_of(st, diff)
}

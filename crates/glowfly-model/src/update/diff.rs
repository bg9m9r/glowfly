//! Diff viewer layout, view mode (hunks or the whole file), navigation and whole-file folds (the
//! hunk view's gaps between hunks are in `diff_gaps`, the fold keys in `fold_keys`).

use crate::effect::Effect;
use crate::msg::DiffMsg;
use crate::state::AppState;
use crate::state::diff_nav::{self, Landing, Step};
use glowfly_core::diff_rows::{header_rows_filled, hunk_at};
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::{DiffLayout, DiffMode, Settings};
use glowfly_core::whole_file::{Expanded, WholeFile, change_of_hunk, change_rows, folds, hunk_of_change, line_at_row, row_of_line, shown};

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
pub fn opening_row(d: &FileDiff, s: &Settings, hunk: Option<usize>, ex: &Expanded) -> usize {
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
    // A step into a collapsed folder opens it, so the revealed row shows.
    if let Some(list) = crate::state::file_folds::list_of(st, &target) {
        st.session.tab_view.file_folds.open_to(list, target.path());
    }
    // After `open_diff`, which clears it.
    st.view.diff_landing = Some((target.clone(), landing));
    super::history_view::follow_step(st, &target, fx);
}

/// A status refresh or a diff-options change arrived while a stepped-to file is still loading:
/// works the step's destination out again from the refreshed list (the shown file's neighbour in
/// the step's direction, or the pending file when the shown one left the list) and requests it
/// with the landing kept. Returns whether it did; otherwise the step is dropped and the caller
/// reloads the shown file as usual.
pub(super) fn renew_step(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let Some((pending, landing)) = st.view.diff_landing.clone() else { return false };
    let shown = st.session.diff.view.as_ref().map(|(t, _)| t.clone());
    let next = match shown {
        Some(s) if diff_nav::in_list(st, &s) => diff_nav::next_file(st, &s, landing.dir()),
        _ if diff_nav::in_list(st, &pending) => Some(pending),
        _ => None,
    };
    match next {
        Some(t) => {
            open_landing(st, t, landing, fx);
            true
        }
        None => {
            st.view.diff_landing = None;
            false
        }
    }
}

/// A file reached by stepping that has nothing to land on (binary, empty, or only whitespace
/// while it is hidden) is passed over when another file follows in the same direction: that one
/// opens instead. Returns whether it did.
pub(super) fn skip_past(st: &mut AppState, target: &DiffTarget, d: &FileDiff, l: Landing, fx: &mut Vec<Effect>) -> bool {
    if !diff_nav::change_rows_of(d, &st.settings, &st.session.tab_view.diff_expanded).is_empty() {
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

pub(super) fn handle(st: &mut AppState, msg: DiffMsg, fx: &mut Vec<Effect>) {
    match msg {
        DiffMsg::SetDiffLayout(layout) => {
            if layout == st.settings.diff_layout {
                return;
            }
            if let Some((_, d)) = &st.session.diff.view {
                let (mode, from, top) = (st.settings.diff_mode, st.settings.diff_layout, st.session.tab_view.diff_top_row);
                let row = same_change_row(d, mode, from, layout, top, &st.session.tab_view.diff_expanded);
                st.view.diff_scroll_to_row = Some(row);
                st.session.tab_view.diff_top_row = row;
                st.view.diff_cursor = None; // the rows are numbered differently now
            }
            // The text selected was on the other layout's rows, or side.
            st.view.diff_text = None;
            st.settings.diff_layout = layout;
            fx.extend(super::prefs::save_settings(st));
        }
        DiffMsg::SetDiffMode(mode) => set_mode(st, mode, fx),
        DiffMsg::DiffHighlighted { target, generation, hunks, whole } => super::highlight::apply(st, target, generation, hunks, whole),
        DiffMsg::SetIgnoreWhitespace(on) => {
            if on == st.settings.diff_ignore_whitespace {
                return;
            }
            st.settings.diff_ignore_whitespace = on;
            st.view.diff_lines.clear();
            // The gaps between the hunks shown now may sit where others did: read them again.
            st.session.tab_view.diff_expanded.drop_gap_text();
            fx.extend(super::prefs::save_settings(st));
            // Hunk rows change, so the diff reopens at the top; hunk operations wait for it, and
            // then count the hunks shown under the new setting.
            super::repo::reopen_diff(st, fx);
        }
        DiffMsg::SetWordWrap(on) => {
            if on == st.settings.diff_word_wrap {
                return;
            }
            st.settings.diff_word_wrap = on;
            // The rows and their indexes stay; only their heights change, so the view keeps
            // the row at the top where it is.
            if st.session.diff.view.is_some() {
                st.view.diff_scroll_to_row = Some(st.session.tab_view.diff_top_row);
            }
            // The conflict view's rows change height too: its current chunk comes back in view.
            if let Some(c) = &st.session.conflict.view {
                st.view.conflict_scroll_to = Some(c.current);
            }
            fx.extend(super::prefs::save_settings(st));
        }
        DiffMsg::SelectDiffLine { .. }
        | DiffMsg::StartLineDrag { .. }
        | DiffMsg::DragDiffLine { .. }
        | DiffMsg::EndLineDrag
        | DiffMsg::ClearLineSelection => lines(st, msg),
        DiffMsg::ClearTextSelection => st.view.diff_text = None,
        DiffMsg::JumpToChange(step) => {
            if st.view.diff_landing.is_some() {
                return; // a stepped-to file is loading: key repeat waits for it
            }
            match diff_nav::step(st, step) {
                Step::Row(row) => {
                    st.view.diff_scroll_to_row = Some(row);
                    // Recorded now so repeated presses before the next frame keep stepping.
                    st.session.tab_view.diff_top_row = row;
                    st.view.diff_cursor = Some(row);
                }
                Step::File(target, landing) => open_landing(st, target, landing, fx),
                Step::End => {}
            }
        }
        // A fold of the whole-file view, or in the hunk view the gap between hunks.
        DiffMsg::ExpandUnchanged(key) if !whole_shown(st) => super::diff_gaps::open(st, key, fx),
        DiffMsg::CollapseUnchanged(key) if !whole_shown(st) => super::diff_gaps::close(st, key),
        DiffMsg::ExpandUnchanged(key) => refold(st, |ex, w| is_fold(w, key) && ex.insert(key)),
        DiffMsg::CollapseUnchanged(key) => refold(st, |ex, w| is_fold(w, key) && ex.remove(key)),
        DiffMsg::OpenFoldAtTop => super::fold_keys::open_at_top(st, fx),
        DiffMsg::CloseFoldAtTop => super::fold_keys::close_at_top(st),
        DiffMsg::GapsLoaded { target, generation, result } => super::diff_gaps::loaded(st, target, generation, result),
        DiffMsg::BinarySidesLoaded { target, generation, result } => super::diff_preview::loaded(st, target, generation, result),
        DiffMsg::ExpandAllUnchanged => refold(st, |ex, w| {
            // Replacing the set also drops keys of folds that are gone (the gaps' text stays).
            let all = Expanded::all(w).with_gap_text_of(ex);
            if *ex == all {
                return false;
            }
            *ex = all;
            true
        }),
        DiffMsg::CollapseAllUnchanged => refold(st, |ex, _| {
            if ex.is_empty() {
                return false;
            }
            ex.clear();
            true
        }),
    }
}

/// Line selection: clicks, and press-and-drag across lines. Clicks and drags are ignored while
/// line operations are blocked.
fn lines(st: &mut AppState, msg: DiffMsg) {
    let blocked = st.line_ops_blocked().is_some();
    let sel = &mut st.view.diff_lines;
    match (msg, &st.session.diff.view) {
        (DiffMsg::ClearLineSelection, _) => sel.clear(),
        (DiffMsg::EndLineDrag, _) => sel.end_drag(),
        _ if blocked => {}
        (DiffMsg::SelectDiffLine { hunk, line, how }, Some((target, d))) => sel.click(target, d, hunk, line, how),
        (DiffMsg::StartLineDrag { hunk, line, how }, Some((target, d))) => sel.start_drag(target, d, hunk, line, how),
        (DiffMsg::DragDiffLine { hunk, line }, Some((target, d))) => sel.drag_to(target, d, hunk, line),
        _ => {}
    }
}

/// Switches between hunks and the whole file, keeping the hunk at the top in view.
fn set_mode(st: &mut AppState, mode: DiffMode, fx: &mut Vec<Effect>) {
    let (old, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    if mode == old {
        return;
    }
    let h = st
        .session
        .diff
        .view
        .as_ref()
        .and_then(|(_, d)| hunk_at_top(d, old, layout, st.session.tab_view.diff_top_row, &st.session.tab_view.diff_expanded));
    st.settings.diff_mode = mode;
    // The hunk view and the whole file name their lines differently.
    st.view.clear_diff_selection();
    fx.extend(super::prefs::save_settings(st));
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
            let Some((_, d)) = st.session.diff.view.as_mut() else { return };
            std::sync::Arc::make_mut(d).whole = None;
            let row = h.and_then(|h| header_rows_filled(d, layout, &st.session.tab_view.diff_expanded).get(h).copied()).unwrap_or(0);
            st.view.diff_scroll_to_row = Some(row);
            st.session.tab_view.diff_top_row = row;
            st.view.diff_cursor = None;
            // Folds opened in the whole file are open gaps here: their text is read.
            super::diff_gaps::diff_shown(st, fx);
        }
    }
}

/// Changes which whole-file folds are open (`change` says whether it changed anything),
/// keeping the line at the top of the viewport there and the change last stepped to in its
/// place among the change rows. Rows above a fold keep their indexes, so opening or closing one
/// fold only scrolls when it begins above the top row; bulk changes can add or remove rows
/// above it. Ignored outside the whole-file view, and when `change` changes nothing (a key
/// that isn't a fold of the file on screen, from a click that raced a reload, or a fold
/// already in the asked-for state).
pub(super) fn refold(st: &mut AppState, change: impl FnOnce(&mut Expanded, &WholeFile) -> bool) {
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let Some((_, d)) = &st.session.diff.view else { return };
    let Some(w) = shown(d, mode) else { return };
    let before = change_rows(d, mode, layout, &st.session.tab_view.diff_expanded);
    let anchor = line_at_row(w, layout, &st.session.tab_view.diff_expanded, st.session.tab_view.diff_top_row);
    if !change(&mut st.session.tab_view.diff_expanded, w) {
        return;
    }
    let after = change_rows(d, mode, layout, &st.session.tab_view.diff_expanded);
    st.view.diff_cursor = st.view.diff_cursor.and_then(|c| before.iter().position(|&r| r == c)).and_then(|i| after.get(i).copied());
    if let Some(line) = anchor {
        let row = row_of_line(w, layout, &st.session.tab_view.diff_expanded, line);
        if row != st.session.tab_view.diff_top_row {
            st.session.tab_view.diff_top_row = row;
            st.view.diff_scroll_to_row = Some(row);
        }
    }
}

/// Whether the whole-file view is on screen.
fn whole_shown(st: &AppState) -> bool {
    st.session.diff.view.as_ref().is_some_and(|(_, d)| shown(d, st.settings.diff_mode).is_some())
}

/// Whether `key` names a fold of `w`.
fn is_fold(w: &WholeFile, key: u32) -> bool {
    folds(w).iter().any(|f| f.key == key)
}

/// The viewer's load of `target`, with the diff settings of now and its list row.
pub(super) fn load_effect(
    st: &AppState,
    workdir: std::path::PathBuf,
    target: DiffTarget,
    reload: bool,
    generation: u64,
    previous: Option<std::sync::Arc<FileDiff>>,
) -> Effect {
    let (ignore_whitespace, whole_file) = (st.settings.diff_ignore_whitespace, st.settings.diff_mode == DiffMode::WholeFile);
    let listed = diff_nav::file_order::listed(st, &target).cloned();
    Effect::LoadDiff { workdir, target, reload, generation, previous, ignore_whitespace, whole_file, listed }
}

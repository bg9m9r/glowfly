//! Moving between blames: Blame before this commit and Back, skipping commits, the whitespace,
//! move and wrap toggles, the jump to the file's history, and the header's Diff and File back to
//! the history the blame was opened over.

use super::reload;
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::blame::{BACK_LIMIT, BlameLanding, BlameStep};
use crate::state::history::{HistoryKind, HistoryPane};
use git2::Oid;
use std::time::Instant;
use trellis_core::settings::BlameMoves;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Reloads the open view keeping its top line in place.
fn reload_in_place(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(top) = st.blame.as_ref().map(|v| v.top_line) else { return };
    reload(st, BlameLanding { line: top, center: false }, now, fx);
}

/// Back to the blame the last step came from, at its scroll position.
pub(super) fn back(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(view) = st.blame.as_mut() else { return };
    let Some(step) = view.back.pop() else { return };
    view.follow = None;
    view.path = step.path;
    view.at = step.at;
    view.skipped = step.skipped;
    view.cursor = step.cursor;
    reload(st, BlameLanding { line: step.top_line, center: false }, now, fx);
}

/// Blames the version just before the commit of line `line`, at the file's path there, and
/// lands on where the line was.
pub(super) fn parent(st: &mut AppState, line: usize, now: Instant, fx: &mut Vec<Effect>) {
    let Some(view) = st.blame.as_mut() else { return };
    let Some(data) = &view.data else { return };
    let Some(origin) = data.blame.origin_of_line(line) else { return };
    let Some(commit) = data.blame.commits.get(origin.commit) else { return };
    if commit.oid.is_zero() {
        return;
    }
    let Some((prev, prev_path)) = origin.previous.clone() else {
        let text = format!("{} added these lines with the file; there is nothing before it", short(commit.oid));
        st.toast(false, text, now);
        return;
    };
    let orig = data.blame.orig_lines.get(line).copied().unwrap_or(0);
    let step =
        BlameStep { path: view.path.clone(), at: view.at, skipped: view.skipped.clone(), top_line: view.top_line, cursor: view.cursor };
    view.back.push(step);
    if view.back.len() > BACK_LIMIT {
        view.back.remove(0);
    }
    view.follow = None;
    view.path = prev_path;
    view.at = Some(prev);
    view.cursor = None;
    reload(st, BlameLanding { line: orig, center: true }, now, fx);
}

/// Blames again looking past `oid`.
pub(super) fn skip(st: &mut AppState, oid: Oid, now: Instant, fx: &mut Vec<Effect>) {
    let Some(view) = st.blame.as_mut() else { return };
    if oid.is_zero() || view.skipped.contains(&oid) {
        return;
    }
    view.skipped.push(oid);
    reload_in_place(st, now, fx);
}

/// Stops skipping commits.
pub(super) fn clear_skipped(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(view) = st.blame.as_mut().filter(|v| !v.skipped.is_empty()) else { return };
    view.skipped.clear();
    reload_in_place(st, now, fx);
}

pub(super) fn set_ignore_whitespace(st: &mut AppState, on: bool, now: Instant, fx: &mut Vec<Effect>) {
    if st.settings.blame.ignore_whitespace == on {
        return;
    }
    st.settings.blame.ignore_whitespace = on;
    saved(st, now, fx);
}

pub(super) fn set_moves(st: &mut AppState, moves: BlameMoves, now: Instant, fx: &mut Vec<Effect>) {
    if st.settings.blame.moves == moves {
        return;
    }
    st.settings.blame.moves = moves;
    saved(st, now, fx);
}

/// Wrap lines: saved, and the top line stays at the top (rows only change height, so there is
/// nothing to blame again).
pub(super) fn set_wrap_lines(st: &mut AppState, on: bool, fx: &mut Vec<Effect>) {
    if st.settings.blame.wrap_lines == on {
        return;
    }
    st.settings.blame.wrap_lines = on;
    fx.push(super::super::prefs::save_settings(st));
    if let Some(view) = st.blame.as_mut() {
        view.scroll_to = Some(BlameLanding { line: view.top_line, center: false });
    }
}

/// After a toggle: saves the settings and blames the open view again.
fn saved(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    fx.push(super::super::prefs::save_settings(st));
    reload_in_place(st, now, fx);
}

/// Closes blame and opens the file's history from the blamed commit, selecting `select`, else
/// the blamed commit.
pub(super) fn open_history(st: &mut AppState, select: Option<Oid>, fx: &mut Vec<Effect>) {
    let Some(view) = st.blame.as_ref() else { return };
    let (path, at) = (view.path.clone(), view.at);
    let blamed = at.or_else(|| view.data.as_ref().map(|d| d.blame.at));
    super::super::history::open(st, path, at, select.or(blamed), HistoryKind::File, fx);
}

/// The header's Diff or File over a file history: closes the blame and shows the history
/// underneath in `pane` (saved, and the File view asks for its text).
pub(super) fn back_to_history(st: &mut AppState, pane: HistoryPane, fx: &mut Vec<Effect>) {
    if st.history.is_none() || st.blame.is_none() {
        return;
    }
    super::close(st);
    super::super::history_view::set_pane(st, pane, fx);
}

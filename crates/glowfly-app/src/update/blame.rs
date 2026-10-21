//! The blame view: opening it on a file (at HEAD or a commit), its result, re-running a HEAD
//! blame when a refresh finds HEAD moved, selecting a line's commit on the graph, and closing
//! it. Stepping back, skipping commits, the header toggles, the jump to file history and the
//! way back to the history underneath are in `blame_nav.rs`; find, go to line and the run keys
//! in `blame_find.rs`.

use crate::effect::{BlameRequest, Effect};
use crate::msg::{BlameMsg, Msg, TrustMsg};
use crate::state::AppState;
use crate::state::blame::{BlameLanding, BlameView};
use glowfly_core::repo::{BlameOptions, Snapshot};
use std::time::Instant;

#[path = "blame_find.rs"]
mod find;
#[cfg(test)]
#[path = "blame_head_tests.rs"]
mod head_tests;
#[path = "blame_nav.rs"]
mod nav;
#[cfg(test)]
#[path = "blame_nav_tests.rs"]
mod nav_tests;

/// Shown instead of a blame while git is held in the repository.
pub(crate) use crate::state::blame::HELD;

pub(super) fn handle(st: &mut AppState, msg: BlameMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        BlameMsg::Open { path, at } => {
            if st.workdir().is_none() {
                return;
            }
            super::conflict::close(st);
            super::issue::close(st);
            // Over the file history, its diff stays underneath; elsewhere no hidden diff stays
            // open for shortcuts to act on.
            if st.history.is_none() {
                super::repo::close_diff(st);
            }
            // Opened on the history's shown file: its header offers the way back to it.
            let over = st.history.as_ref().and_then(|h| h.shown()).is_some_and(|(oid, p, _)| at == Some(oid) && p == path);
            if !st.runs_git_unasked() {
                st.blame = Some(BlameView { over_history: over, ..BlameView::failed(path, at, HELD) });
                hold(st, now, fx);
                return;
            }
            st.blame = Some(BlameView { over_history: over, ..BlameView::loading(path, at) });
            request(st, fx);
        }
        BlameMsg::Loaded { path, generation, result } => loaded(st, path, generation, result),
        BlameMsg::Close => close(st),
        BlameMsg::SelectCommit(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                super::repo::select_commit(st, oid, true, fx);
            } else {
                st.toast(false, super::history::not_on_graph(oid), now);
            }
        }
        BlameMsg::Back => nav::back(st, now, fx),
        BlameMsg::BlameParent { line } => nav::parent(st, line, now, fx),
        BlameMsg::SkipCommit(oid) => nav::skip(st, oid, now, fx),
        BlameMsg::ClearSkipped => nav::clear_skipped(st, now, fx),
        BlameMsg::SetIgnoreWhitespace(on) => nav::set_ignore_whitespace(st, on, now, fx),
        BlameMsg::SetMoves(m) => nav::set_moves(st, m, now, fx),
        BlameMsg::SetWrapLines(on) => nav::set_wrap_lines(st, on, fx),
        BlameMsg::OpenHistory { select } => nav::open_history(st, select, fx),
        BlameMsg::BackToHistory(pane) => nav::back_to_history(st, pane, fx),
        BlameMsg::FindChanged => find::find_changed(st),
        BlameMsg::FindStep(d) => find::find_step(st, d),
        BlameMsg::FocusFind => find::focus_find(st),
        BlameMsg::GoToLine => find::go_to_line(st),
        BlameMsg::StepRun(d) => find::step_run(st, d),
        BlameMsg::Retry => {
            let Some(view) = st.blame.as_ref().filter(|v| !v.loading) else { return };
            let land = BlameLanding { line: view.cursor.unwrap_or(view.top_line), center: view.cursor.is_some() };
            reload(st, land, now, fx);
        }
    }
}

fn loaded(st: &mut AppState, path: String, generation: u64, result: Result<Box<crate::state::blame::BlameData>, String>) {
    if generation != st.blame_gen {
        return; // superseded: closed, reopened, or parked with its tab
    }
    let Some(view) = st.blame.as_mut().filter(|v| v.path == path) else { return };
    view.loading = false;
    let land = view.land.take();
    view.follow = None;
    match result {
        Ok(d) => {
            let n = d.blame.lines.len();
            view.data_gen = generation;
            if view.at.is_none() && d.blame.path != view.path {
                view.path = d.blame.path.clone(); // followed through a rename since HEAD moved
            }
            view.cursor = view.cursor.filter(|&c| c < n);
            if !view.find.query.is_empty() {
                view.find.hits = crate::state::blame_find::search(&d.blame.lines, &view.find.query);
                view.find.current = None;
            }
            view.data = Some(*d);
            if let Some(l) = land.filter(|_| n > 0) {
                let line = l.line.min(n - 1);
                view.scroll_to = Some(BlameLanding { line, center: l.center });
                if l.center {
                    view.cursor = Some(line);
                }
            }
        }
        Err(e) => {
            view.data = None; // the error replaces a blame a re-run kept on screen
            view.error = Some(e.clone());
            st.log(false, e);
        }
    }
}

/// Blames the open view again (after a step, a skip or a toggle) and scrolls to `land` when it
/// arrives. In a held repository the view shows why instead and trust is asked for.
pub(super) fn reload(st: &mut AppState, land: BlameLanding, now: Instant, fx: &mut Vec<Effect>) {
    let held = !st.runs_git_unasked();
    let Some(view) = st.blame.as_mut() else { return };
    view.data = None;
    view.error = None;
    view.land = Some(land);
    if held {
        hold(st, now, fx);
        return;
    }
    view.loading = true;
    request(st, fx);
}

/// A refresh brought snapshot `new`: when HEAD moved (a commit, checkout, pull, reset, ...) a
/// blame at HEAD runs again, following the file through renames since the HEAD it was shown
/// at. The old blame stays on screen until the result replaces it, keeping the top line. A
/// blame at a fixed commit and a held repository are left alone. Called before `st.snap` is
/// replaced.
pub(super) fn head_moved(st: &mut AppState, new: &Snapshot, fx: &mut Vec<Effect>) {
    let Some(old) = st.snap.as_ref().map(|s| s.head_oid).filter(|h| *h != new.head_oid) else { return };
    if !st.runs_git_unasked() {
        return;
    }
    let Some(view) = st.blame.as_mut().filter(|v| v.at.is_none()) else { return };
    view.follow = view.follow.or(old); // the earliest HEAD while re-runs stack up
    if view.land.is_none() {
        view.land = Some(BlameLanding { line: view.top_line, center: false });
    }
    view.loading = true;
    view.error = None;
    request(st, fx);
}

/// Shows HELD in the open view and asks to trust the repository.
fn hold(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(view) = st.blame.as_mut() {
        view.loading = false;
        view.error = Some(HELD.to_owned());
    }
    st.blame_gen += 1;
    if let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) {
        fx.extend(super::update(st, Msg::Trust(TrustMsg::Ask { real }), now));
    }
}

/// Asks for the open view's blame; results of older requests are ignored from now on.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.blame_gen += 1;
    if let (Some(workdir), Some(view)) = (st.workdir(), &st.blame) {
        let opts = BlameOptions {
            ignore_whitespace: st.settings.blame.ignore_whitespace,
            moves: st.settings.blame.moves,
            ignore_revs: view.skipped.clone(),
        };
        fx.push(Effect::LoadBlame(BlameRequest {
            workdir,
            path: view.path.clone(),
            at: view.at,
            opts,
            highlight: st.settings.syntax_highlighting,
            generation: st.blame_gen,
            follow: view.follow,
        }));
    }
}

/// Closes the view; a request still in flight must not reopen it.
pub(super) fn close(st: &mut AppState) {
    if st.blame.take().is_some() {
        st.blame_gen += 1;
    }
}

/// After a tab switch: a blame dropped while the tab was parked is asked for again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.blame.as_ref().is_some_and(|v| v.loading) {
        request(st, fx);
    }
}

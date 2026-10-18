//! The blame view: opening it on a file (at HEAD or a commit), its result, selecting a line's
//! commit on the graph, and closing it. Stepping back, skipping commits, the header toggles and
//! the jump to file history are in `blame_nav.rs`; find, go to line and the run keys in
//! `blame_find.rs`.

use crate::effect::{BlameRequest, Effect};
use crate::msg::{BlameMsg, Msg, TrustMsg};
use crate::state::AppState;
use crate::state::blame::{BlameLanding, BlameView};
use std::time::Instant;
use trellis_core::repo::BlameOptions;

#[path = "blame_find.rs"]
mod find;
#[path = "blame_nav.rs"]
mod nav;
#[cfg(test)]
#[path = "blame_nav_tests.rs"]
mod nav_tests;

/// Shown instead of a blame while git is held in the repository.
pub(crate) const HELD: &str = "Blame runs git in this repository. Trust it first (the Untrusted chip in the toolbar).";

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
            if !st.runs_git_unasked() {
                st.blame = Some(BlameView::failed(path, at, HELD));
                hold(st, now, fx);
                return;
            }
            st.blame = Some(BlameView::loading(path, at));
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
        BlameMsg::OpenHistory { select } => nav::open_history(st, select, fx),
        BlameMsg::FindChanged => find::find_changed(st),
        BlameMsg::FindStep(d) => find::find_step(st, d),
        BlameMsg::FocusFind => find::focus_find(st),
        BlameMsg::GoToLine => find::go_to_line(st),
        BlameMsg::StepRun(d) => find::step_run(st, d),
    }
}

fn loaded(st: &mut AppState, path: String, generation: u64, result: Result<Box<crate::state::blame::BlameData>, String>) {
    if generation != st.blame_gen {
        return; // superseded: closed, reopened, or parked with its tab
    }
    let Some(view) = st.blame.as_mut().filter(|v| v.path == path) else { return };
    view.loading = false;
    let land = view.land.take();
    match result {
        Ok(d) => {
            let n = d.blame.lines.len();
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

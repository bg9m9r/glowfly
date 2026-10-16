//! The blame view: opening it on a file (at HEAD or a commit), its result, selecting a line's
//! commit on the graph, and closing it.

use crate::effect::{BlameRequest, Effect};
use crate::msg::{BlameMsg, Msg, TrustMsg};
use crate::state::AppState;
use crate::state::blame::BlameView;
use std::time::Instant;

/// Shown instead of a blame while git is held in the repository.
pub(crate) const HELD: &str = "Blame runs git in this repository. Trust it first (the Untrusted chip in the toolbar).";

pub(super) fn handle(st: &mut AppState, msg: BlameMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        BlameMsg::Open { path, at } => {
            if st.workdir().is_none() {
                return;
            }
            super::conflict::close(st);
            // Over the file history, its diff stays underneath; elsewhere no hidden diff stays
            // open for shortcuts to act on.
            if st.history.is_none() {
                super::repo::close_diff(st);
            }
            if !st.runs_git_unasked() {
                st.blame = Some(BlameView::failed(path, at, HELD));
                st.blame_gen += 1;
                if let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) {
                    fx.extend(super::update(st, Msg::Trust(TrustMsg::Ask { real }), now));
                }
                return;
            }
            st.blame = Some(BlameView::loading(path, at));
            request(st, fx);
        }
        BlameMsg::Loaded { path, generation, result } => {
            if generation != st.blame_gen {
                return; // superseded: closed, reopened, or parked with its tab
            }
            let Some(view) = st.blame.as_mut().filter(|v| v.path == path) else { return };
            view.loading = false;
            match result {
                Ok(d) => view.data = Some(*d),
                Err(e) => {
                    view.error = Some(e.clone());
                    st.log(false, e);
                }
            }
        }
        BlameMsg::Close => close(st),
        BlameMsg::SelectCommit(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                super::repo::select_commit(st, oid, true, fx);
            } else {
                st.toast(false, super::history::not_on_graph(oid), now);
            }
        }
    }
}

/// Asks for the open view's blame; results of older requests are ignored from now on.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.blame_gen += 1;
    if let (Some(workdir), Some(view)) = (st.workdir(), &st.blame) {
        fx.push(Effect::LoadBlame(BlameRequest {
            workdir,
            path: view.path.clone(),
            at: view.at,
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

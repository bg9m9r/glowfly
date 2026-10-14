//! File history: opening the commits that changed a file, selecting one (which opens its diff
//! of the file), searching further back, and jumping to a commit on the graph.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::history::FileHistoryView;
use std::time::Instant;
use trellis_core::repo::{DiffTarget, HistoryStart};

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::OpenFileHistory { path, at } => {
            if st.workdir().is_none() {
                return;
            }
            super::repo::close_diff(st);
            super::conflict::close(st);
            st.history = Some(FileHistoryView::loading(path, at));
            request(st, at.map_or(HistoryStart::Head, HistoryStart::At), fx);
        }
        Msg::FileHistoryLoaded { path, generation, result } => {
            if generation != st.history_gen {
                return; // superseded: closed, reopened, or parked with its tab
            }
            let Some(view) = st.history.as_mut().filter(|v| v.path == path) else { return };
            match result {
                Ok(h) => {
                    let first = view.entries.is_empty();
                    if first {
                        view.as_of = if h.from_head { None } else { view.at };
                    }
                    view.append(h);
                    if first && !view.entries.is_empty() {
                        select(st, 0, fx);
                    }
                }
                Err(e) => {
                    view.loading = false;
                    view.error = Some(e.clone());
                    st.log(false, e);
                }
            }
        }
        Msg::SelectHistoryEntry(i) => select(st, i, fx),
        Msg::MoreFileHistory => {
            let Some(view) = st.history.as_mut().filter(|v| !v.loading && !v.resume.is_empty()) else { return };
            view.loading = true;
            view.error = None;
            let start = HistoryStart::Resume(view.resume.clone());
            request(st, start, fx);
        }
        Msg::CloseFileHistory => {
            close(st);
            super::repo::close_diff(st);
        }
        Msg::ShowInGraph(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                close(st);
                super::repo::select_commit(st, oid, true, fx);
            } else {
                let text = format!(
                    "{} is not visible on the graph: its branch is hidden or it is older than the commit limit",
                    &oid.to_string()[..7]
                );
                st.toast(false, text, now);
            }
        }
        _ => {}
    }
}

/// Selects entry `i` and opens that commit's diff of the file (at its path then).
fn select(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(view) = st.history.as_mut() else { return };
    let Some(e) = view.entries.get(i) else { return };
    let target = DiffTarget::Commit(e.oid, e.path.clone());
    view.selected = Some(i);
    st.view.history_scroll_to = Some(i);
    super::repo::open_diff(st, target, fx);
}

/// Starts a walk; results of older requests are ignored from now on.
fn request(st: &mut AppState, start: HistoryStart, fx: &mut Vec<Effect>) {
    st.history_gen += 1;
    if let (Some(workdir), Some(view)) = (st.workdir(), &st.history) {
        fx.push(Effect::LoadFileHistory {
            workdir,
            path: view.path.clone(),
            start,
            limit: st.settings.commit_limit(),
            generation: st.history_gen,
        });
    }
}

/// Closes the view (not the diff); a request still in flight must not reopen it.
pub(super) fn close(st: &mut AppState) {
    if st.history.take().is_some() {
        st.history_gen += 1;
    }
}

/// After a tab switch: a request dropped while the tab was parked is sent again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = st.history.as_ref().filter(|v| v.loading) else { return };
    let start = if view.entries.is_empty() {
        view.at.map_or(HistoryStart::Head, HistoryStart::At)
    } else {
        HistoryStart::Resume(view.resume.clone())
    };
    request(st, start, fx);
}

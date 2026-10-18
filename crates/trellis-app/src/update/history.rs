//! File history: opening the commits that changed a file, selecting one (which opens its diff
//! of the file), searching further back, and jumping to a commit on the graph.

use crate::effect::Effect;
use crate::msg::HistoryMsg;
use crate::state::AppState;
use crate::state::history::FileHistoryView;
use std::time::Instant;
use trellis_core::repo::{DiffTarget, HistoryStart};

pub(super) fn handle(st: &mut AppState, msg: HistoryMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        HistoryMsg::OpenFileHistory { path, at } => open(st, path, at, None, fx),
        HistoryMsg::FileHistoryLoaded { path, generation, result } => {
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
                        let want = view.select.take();
                        let i = want.and_then(|oid| view.entries.iter().position(|e| e.oid == oid)).unwrap_or(0);
                        select(st, i, fx);
                    }
                }
                Err(e) => {
                    view.loading = false;
                    view.error = Some(e.clone());
                    st.log(false, e);
                }
            }
        }
        HistoryMsg::SelectHistoryEntry(i) => select(st, i, fx),
        HistoryMsg::MoreFileHistory => {
            let Some(view) = st.history.as_mut().filter(|v| !v.loading && !v.resume.is_empty()) else { return };
            view.loading = true;
            view.error = None;
            let start = HistoryStart::Resume(view.resume.clone());
            request(st, start, fx);
        }
        HistoryMsg::CloseFileHistory => {
            close(st);
            super::repo::close_diff(st);
        }
        HistoryMsg::ShowInGraph(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                close(st);
                super::repo::select_commit(st, oid, true, fx);
            } else {
                st.toast(false, not_on_graph(oid), now);
            }
        }
    }
}

/// Opens the history of `path` from `at` (HEAD when None), closing the views it replaces; the
/// first batch selects `select` when it lists it, else its first entry.
pub(super) fn open(st: &mut AppState, path: String, at: Option<git2::Oid>, select: Option<git2::Oid>, fx: &mut Vec<Effect>) {
    if st.workdir().is_none() {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::blame::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    let mut view = FileHistoryView::loading(path, at);
    view.select = select;
    st.history = Some(view);
    request(st, at.map_or(HistoryStart::Head, HistoryStart::At), fx);
}

/// Why commit `oid` can't be selected on the graph.
pub(super) fn not_on_graph(oid: git2::Oid) -> String {
    format!("{} is not visible on the graph: its branch is hidden or it is older than the commit limit", &oid.to_string()[..7])
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

/// The selected entry's diff, when it is not the diff on screen.
pub(super) fn missing_diff(st: &AppState) -> Option<DiffTarget> {
    let view = st.history.as_ref()?;
    let e = view.entries.get(view.selected?)?;
    let target = DiffTarget::Commit(e.oid, e.path.clone());
    st.diff.as_ref().is_none_or(|(t, _)| *t != target).then_some(target)
}

/// After a tab switch: requests dropped while the tab was parked (the walk, and the selected
/// entry's diff) are sent again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(target) = missing_diff(st) {
        super::repo::open_diff(st, target, fx);
    }
    let Some(view) = st.history.as_ref().filter(|v| v.loading) else { return };
    let start = if view.entries.is_empty() {
        view.at.map_or(HistoryStart::Head, HistoryStart::At)
    } else {
        HistoryStart::Resume(view.resume.clone())
    };
    request(st, start, fx);
}

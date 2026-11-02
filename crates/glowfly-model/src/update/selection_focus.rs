//! The Go back to the focused commit key: from a multi-selection (after select all, Shift-ranges
//! or Ctrl-clicks), select only the commit in focus, scroll the graph to it and say how many
//! commits were let go; and Ctrl+C in the graph, copying the selected commits' full IDs.

use crate::effect::{Effect, UiEffect};
use crate::state::journal::short;
use crate::state::{AppState, Selection};
use std::time::Instant;

pub(super) fn back(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    match &st.session.selection {
        Selection::Commits(m) => {
            let (focus, dropped) = (m.focus(), m.len() - 1);
            super::repo::select_commit(st, focus, true, fx);
            let t = st.settings.lang();
            st.toast(true, t.trn("details.multi.back.toast", dropped, &[("sha", &short(focus))]), now);
        }
        // Like Esc: back to the compared commit, with nothing to say.
        Selection::VsWorkdir(w) => {
            let oid = w.oid;
            super::repo::select_commit(st, oid, false, fx);
        }
        Selection::None | Selection::Wip | Selection::Commit(_) => {}
    }
}

/// Copies the selected commits' full IDs, one per line, newest first, and says how many.
pub(super) fn copy_ids(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let oids = match &st.session.selection {
        Selection::Commit(o) => vec![*o],
        Selection::Commits(m) => crate::state::selection::in_graph_order(m.oids.iter().copied(), snap, false),
        Selection::VsWorkdir(w) => vec![w.oid],
        Selection::None | Selection::Wip => return,
    };
    let text = oids.iter().map(|o| o.to_string()).collect::<Vec<_>>().join("\n");
    fx.push(Effect::Ui(UiEffect::CopyText(text)));
    let t = st.settings.lang();
    st.toast(true, t.trn("graph.copy_ids.toast", oids.len(), &[]), now);
}

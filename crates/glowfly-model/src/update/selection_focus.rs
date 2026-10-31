//! The Go back to the focused commit key: from a multi-selection (after select all, Shift-ranges
//! or Ctrl-clicks), select only the commit in focus, scroll the graph to it and say how many
//! commits were let go.

use crate::effect::Effect;
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

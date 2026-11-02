//! Freeing a long-parked tab's history: a tab parked `HISTORY_FREE_AFTER` with
//! `HISTORY_FREE_MIN_COMMITS` or more loaded keeps a stub of its snapshot (refs, status, workdir
//! and what the tab and the sidebar show) and hands its commits, their rows and the graph's layout
//! to a worker to free (`tick`). Everything else parks as before: the selection, the open diff,
//! the draft, the scroll.
//!
//! Shown again, the stub's tab reloads its history in full, as deep as it had been loaded
//! (`refresh`), and a refresh asked for meanwhile stays a full one (`refresh::request`). Until it
//! lands the graph shows a spinner and keeps the scroll to restore; the reload is a refresh, so a
//! selected commit still in the history stays selected. The search index has its own freeing
//! (`history_index::tick`).

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::tabs::Parked;
use std::sync::Arc;
use std::time::Instant;

/// A tick at `t`: dates every park no tick has seen, and frees the history of each tab parked
/// long enough with a large one loaded.
pub(super) fn tick(st: &mut AppState, t: Instant, fx: &mut Vec<Effect>) {
    for tab in &mut st.tabs {
        let Some(p) = tab.parked.as_deref_mut() else { continue };
        p.since.get_or_insert(t);
        if p.history_free_at().is_some_and(|at| t >= at) {
            free(p, fx);
        }
    }
}

/// Leaves `p` a stub of its snapshot: no commits, rows, graph or walk to go on from.
fn free(p: &mut Parked, fx: &mut Vec<Effect>) {
    let Some(snap) = p.snap.as_mut() else { return };
    tracing::debug!(workdir = %snap.workdir.display(), commits = snap.commits.len(), "freeing a long-parked tab's history");
    p.session.tab_view.history_freed = Some(snap.loaded_with.max_commits);
    // Rows of the freed history: matched again when it is back (`search::snapshot_changed`).
    p.session.search.matches.clear();
    let commits = std::mem::replace(&mut snap.commits, Arc::from(Vec::new()));
    let freed = (commits, std::mem::take(&mut snap.row_of), std::mem::take(&mut snap.graph), snap.frontier.take());
    super::large_graph::free(fx, "parked history", Box::new(freed));
}

/// What the active tab asks for on being shown: a full reload of a freed history, as deep as
/// it was loaded; otherwise the refresh that only walks the history again if the refs moved.
pub(super) fn refresh(st: &mut AppState, fx: &mut Vec<Effect>) {
    match st.session.tab_view.history_freed {
        Some(depth) => {
            st.refresh.extra_commits = depth.saturating_sub(st.settings.commit_limit());
            super::refresh::reload(st, fx);
        }
        None => super::repo::refresh(st, fx),
    }
}

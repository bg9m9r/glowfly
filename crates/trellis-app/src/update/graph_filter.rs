//! Hidden and soloed refs in the graph, saved per repository: changing them saves the settings
//! and reloads; each load drops refs that no longer exist. Also the rows highlighted for the ref
//! under the pointer.

use crate::effect::{Effect, LoadKind};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::branches::HEAD_ALWAYS_SHOWN;
use trellis_core::repo::Snapshot;
use trellis_core::settings::FilterOp;

pub(super) fn handle(st: &mut AppState, msg: Msg, fx: &mut Vec<Effect>) {
    let Msg::GraphFilter(op) = msg else { return };
    let Some(w) = st.workdir() else { return };
    // HEAD is always walked, so hiding its branch would only drop the label. The menu disables
    // the item; a late message is refused with the same reason.
    let head = st.snap.as_ref().and_then(|s| s.head_branch.as_deref()).map(|b| format!("refs/heads/{b}"));
    if let FilterOp::Hide(r) = &op
        && head.as_ref() == Some(r)
    {
        st.log(false, HEAD_ALWAYS_SHOWN);
        return;
    }
    let mut f = st.settings.graph_filter_for(&w);
    if !f.apply(op) {
        return;
    }
    st.settings.set_graph_filter(&w, f);
    fx.push(super::prefs::save_settings(st));
    super::repo::refresh(st, fx);
}

/// A repository finished loading (before `snap` becomes the active snapshot). Drops hidden and
/// soloed refs that no longer exist from the saved filter, and returns whether the repository
/// must be loaded again: a soloed ref went away (the graph would show only HEAD), or it was
/// opened through another spelling of its path, so the saved filter wasn't applied.
pub(super) fn loaded(st: &mut AppState, snap: &Snapshot, kind: LoadKind, fx: &mut Vec<Effect>) -> bool {
    let mut f = st.settings.graph_filter_for(&snap.workdir);
    let (mut hidden_changed, solo_changed) = f.prune(|r| ref_exists(snap, r));
    // A hidden branch that has since been checked out is shown again (the load already showed it).
    if let Some(b) = &snap.head_branch {
        hidden_changed |= f.apply(FilterOp::Show(format!("refs/heads/{b}")));
    }
    if hidden_changed || solo_changed {
        st.settings.set_graph_filter(&snap.workdir, f);
        fx.push(super::prefs::save_settings(st));
    }
    // The tab still holds the requested path: `tabs::opened` replaces it after this.
    let respelled = kind == LoadKind::Open
        && st
            .tabs
            .get(st.active_tab)
            .and_then(|t| t.path.as_ref())
            .is_some_and(|requested| st.settings.graph_filter_for(requested) != st.settings.graph_filter_for(&snap.workdir));
    solo_changed || respelled
}

/// Whether the full ref name `r` is one of the snapshot's branches or tags.
fn ref_exists(snap: &Snapshot, r: &str) -> bool {
    if let Some(name) = r.strip_prefix("refs/heads/") {
        snap.locals.iter().any(|b| b.name == name)
    } else if let Some(rest) = r.strip_prefix("refs/remotes/") {
        snap.remotes.iter().any(|(remote, branches)| branches.iter().any(|(b, _)| format!("{remote}/{b}") == rest))
    } else if let Some(name) = r.strip_prefix("refs/tags/") {
        snap.tags.iter().any(|(t, _)| t == name)
    } else {
        // Nothing else can be hidden or soloed; leave what this can't judge.
        true
    }
}

/// Highlights the rows of the branch whose tip is `tip` (none for `None`). The default branch's
/// history is walked once per snapshot, on the first hover.
pub(super) fn hover_ref(st: &mut AppState, tip: Option<git2::Oid>) {
    let (Some(tip), Some(snap)) = (tip, st.snap.as_ref()) else {
        st.view.highlight = None;
        return;
    };
    let base = st.view.base_set.get_or_insert_with(|| crate::state::ref_rows::default_ancestry(snap).1);
    st.view.highlight = Some((tip, crate::state::ref_rows::branch_rows(snap, tip, base)));
}

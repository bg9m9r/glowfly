//! Hidden and soloed refs in the graph, saved per repository: changing them saves the settings
//! and reloads; each load drops refs that no longer exist. Also the rows highlighted for the ref
//! under the pointer, and the off-thread results for a large history's graph (large_graph.rs).

use crate::effect::{Effect, LoadKind};
use crate::msg::GraphMsg;
use crate::state::AppState;
use crate::state::branches::HEAD_ALWAYS_SHOWN;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::FilterOp;
use std::sync::Arc;

pub(super) fn handle(st: &mut AppState, msg: GraphMsg, fx: &mut Vec<Effect>) {
    match msg {
        GraphMsg::GraphFilter(op) => filter(st, op, fx),
        GraphMsg::HoverRef(tip) => hover_ref(st, tip, fx),
        GraphMsg::LaidOut { request, fit, layout } => super::large_graph::laid_out(st, request, &fit, *layout, fx),
        GraphMsg::AncestryFound { request, base } => super::large_graph::ancestry_found(st, request, base),
        GraphMsg::PinColour { key, slot } => super::branch_colour::pin(st, &key, slot, fx),
        GraphMsg::ResizeColumn { col, width } => super::graph_columns::resize(st, col, width, fx),
        GraphMsg::ResetColumnWidth(col) => super::graph_columns::reset(st, col, fx),
    }
}

fn filter(st: &mut AppState, op: FilterOp, fx: &mut Vec<Effect>) {
    let Some(w) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
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
/// opened through another spelling of its path, so the saved filter wasn't applied. Settings
/// earlier versions saved under libgit2's spelling move to the resolved path first.
pub(super) fn loaded(st: &mut AppState, snap: &Snapshot, kind: LoadKind, fx: &mut Vec<Effect>) -> bool {
    let aliases: Vec<&std::path::Path> =
        [Some(&snap.workdir), snap.opened_as.as_ref()].into_iter().flatten().map(|p| p.as_path()).collect();
    let adopted = st.settings.adopt_repo_keys(&snap.real_workdir, &aliases);
    let saved = st.settings.graph_filter_for(&snap.real_workdir);
    let mut f = saved.clone();
    let (mut hidden_changed, solo_changed) = f.prune(|r| ref_exists(snap, r));
    // A hidden branch that has since been checked out is shown again (the load already showed it).
    if let Some(b) = &snap.head_branch {
        hidden_changed |= f.apply(FilterOp::Show(format!("refs/heads/{b}")));
    }
    if hidden_changed || solo_changed {
        st.settings.set_graph_filter(&snap.real_workdir, f);
    }
    if adopted || hidden_changed || solo_changed {
        fx.push(super::prefs::save_settings(st));
    }
    // Loaded with the filter saved for another spelling (the path it was opened by, before its
    // resolved form was known): load again with its own.
    let respelled = kind == LoadKind::Open && (snap.loaded_with.hidden != saved.hidden || snap.loaded_with.solo != saved.solo);
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
/// history is walked once per snapshot, on the first hover (on a worker for a large history,
/// highlighting once it answers), and each tip's rows once per snapshot.
fn hover_ref(st: &mut AppState, tip: Option<git2::Oid>, fx: &mut Vec<Effect>) {
    let Some(tip) = tip.filter(|_| st.snap.is_some()) else {
        st.view.highlight = None;
        return;
    };
    if !super::large_graph::ancestry_ready(st, fx) {
        st.view.highlight = None;
        return;
    }
    let (Some(snap), rows) = (st.snap.as_ref(), &mut st.view.ref_rows) else { return };
    let Some(base) = &rows.base else { return };
    let branch = rows.rows.entry(tip).or_insert_with(|| Arc::new(crate::state::ref_rows::branch_rows(snap, tip, base)));
    st.view.highlight = Some((tip, Arc::clone(branch)));
}

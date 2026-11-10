//! A repository load's result: the new snapshot taken in (opening or a refresh), or a failed
//! load (held for load more, kept for a freed tab's pane, a failed open closing its tab).

use super::repo::{close_diff, reload_diff};
use crate::effect::{Effect, LoadKind, SaveEffect, UiEffect};
use crate::state::{AppState, CommitDraft, Selection};
use glowfly_core::repo::Snapshot;
use std::time::Instant;

pub(super) fn finished(
    st: &mut AppState,
    generation: u64,
    kind: LoadKind,
    result: Result<Box<Snapshot>, String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if generation != st.load_gen {
        return; // superseded by a newer load
    }
    st.loading = false;
    let read = result.is_ok();
    match result {
        Ok(snap) => {
            st.refresh.more_retries = 0;
            loaded(st, *snap, kind, now, fx)
        }
        Err(e) => {
            // The snapshot is unchanged, so a load for more is still wanted.
            if st.loading_more() {
                super::load_more::failed(st, e, now);
            } else {
                super::parked_free::load_failed(st, e);
            }
            if kind == LoadKind::Open {
                let path = st.tabs.get(st.active_tab).and_then(|t| t.path.clone());
                super::tabs::open_failed(st, fx);
                if let Some(path) = path {
                    super::recent::open_failed(st, path, fx);
                }
            }
            super::push_pr::load_failed(st, fx);
        }
    }
    super::conflict_count::status_read(st, super::conflict_count::Coverage::whole(read));
    super::refresh::load_finished(st, fx);
}

fn loaded(st: &mut AppState, mut snap: Snapshot, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    let anchor = super::graph_anchor::take(st, kind);
    let state_before = st.snap.as_ref().filter(|_| kind == LoadKind::Refresh).map(|s| s.state);
    st.known.learn_snapshot(&snap);
    if kind == LoadKind::Open && super::tabs::open_elsewhere(st, &snap, fx) {
        super::deep_link_open::ready(st, now, fx);
        return;
    }
    st.cues.ref_rows.reset();
    st.cues.highlight = None;
    match kind {
        LoadKind::Open => {
            // Any spelling of this folder (a short 8.3 name, a link, another case where the volume ignores it) gives way to libgit2's.
            let recent = std::mem::take(&mut st.recent);
            st.recent = recent.into_iter().filter(|p| !st.names_repo(&snap, p)).collect();
            st.recent.insert(0, snap.workdir.clone());
            st.recent.truncate(crate::state::repo_lists::RECENT_LIMIT);
            fx.push(Effect::Save(SaveEffect::Recent(st.recent.clone())));
            let missing = std::mem::take(&mut st.missing_repos);
            st.missing_repos = missing.into_iter().filter(|p| !st.names_repo(&snap, p)).collect();
            let label = st.tabs.get(st.active_tab).and_then(|t| t.label.as_deref());
            fx.push(Effect::Ui(UiEffect::SetTitle(format!("{} - Glowfly", label.unwrap_or(&snap.name)))));
            st.session.selection = if snap.is_dirty() { Selection::Wip } else { Selection::None };
            st.session.wip_lit = false;
            st.session.detail = None;
            close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            super::pull_request::close(st);
            super::issue::close(st);
            super::rebase::close(st);
            super::journal::close(st);
            st.session.gitflow_panel = false;
            st.session.gitflow_resume = None;
            st.session.draft = CommitDraft::default();
            st.session.undo = Default::default();
            st.view.scroll_to_row = Some(0);
            super::tabs::forget_repo_reads(st);
        }
        LoadKind::Refresh | LoadKind::More => {
            // (A load for more that loaded everything comes back as a refresh: runner/load.rs.)
            super::history::head_moved(st, &snap, fx);
            super::blame::head_moved(st, &snap, fx);
            if let Selection::Commit(oid) = st.session.selection {
                // A pull request's commit (a fork's head, say) may be on no branch of the graph.
                let in_pr = st.session.pull_request.view.as_ref().is_some_and(|v| v.has_commit(oid));
                let still_there = snap.row_of.contains_key(&oid) || snap.stashes.iter().any(|s| s.oid == oid) || in_pr;
                if !still_there {
                    st.session.selection = Selection::None;
                    st.session.detail = None;
                }
            }
            if matches!(st.session.selection, Selection::Commits(_)) {
                super::selection::refreshed(st, &snap, fx);
            }
            super::compare_wip::refreshed(st, &snap, fx);
            super::rebase::refreshed(st, &snap);
            super::stack::refreshed(st, &snap, fx);
            super::shallow::refreshed(st, &snap, fx);
        }
    }
    super::forge::detect(st, &snap);
    let reload = super::graph_filter::loaded(st, &snap, kind, fx);
    super::notify::loaded(st, &snap, kind, now, fx);
    // After the filter, which adopts the repository's keys: the pins are found under its own.
    super::branch_colour::apply(&st.settings, st.cues.trunk_near, &mut snap);
    super::large_graph::show(st, snap, fx);
    super::line_counts::status_changed(st, fx);
    super::pick::state_changed(st, state_before);
    // Before the auto-stash: remembered Git Flow steps dropped by this load release its stash.
    super::gitflow::snapshot_changed(st);
    super::auto_stash::loaded(st, kind, now, fx);
    super::search::snapshot_changed(st, fx);
    super::trust::loaded(st, kind, fx);
    super::custom_commands::loaded(st, fx);
    super::submodule_batch::opened(st, kind, now, fx);
    super::conflict_check::want(st, fx);
    super::worktree_counts::loaded(st, now, fx);
    super::commit::prefill(st);
    super::watch::sync(st, fx);
    super::terminal::sync(st, fx);
    if kind == LoadKind::Open {
        fx.push(super::tabs::opened(st));
        super::conflict_files::want(st, fx);
        super::profile_rules::want_identity(st, fx);
    } else {
        super::conflict::status_changed(st, now, fx);
    }
    super::pull_request::heads::resolve_pr_heads(st);
    if kind == LoadKind::Refresh {
        super::pull_request::refreshed(st, fx);
        super::journal::refreshed(st, fx);
    }
    reload_diff(st, fx);
    super::forge::refresh(st, kind == LoadKind::Open, false, now, fx);
    super::push_pr::loaded(st, fx);
    super::onboarding::loaded(st, kind, fx);
    let slow = st.snap.as_ref().and_then(|s| s.slow_status.clone());
    super::status_tip::check(st, slow.as_ref(), now, fx);
    super::deep_link_open::ready(st, now, fx);
    super::graph_anchor::restore(st, anchor);
    if reload {
        super::refresh::reload_after(st, fx);
    }
}

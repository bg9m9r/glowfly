//! Opening and refreshing the repository, selection, diffs and search.

use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::{FolderPurpose, Msg};
use crate::state::{AppState, CommitDraft, Selection};
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::Snapshot;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::PickRepo => fx.push(Effect::PickFolder { title: "Open a Git repository".into(), purpose: FolderPurpose::OpenRepo }),
        Msg::Open(path) => super::tabs::open(st, path, fx),
        Msg::Refresh => {
            refresh(st, fx);
            super::forge::refresh(st, true, false, now, fx);
        }
        Msg::WindowFocused => {
            // Pick up changes made outside the app.
            if st.job.is_none() {
                refresh(st, fx);
            }
        }
        Msg::RepoLoaded { generation, kind, result } => {
            if generation != st.load_gen {
                return; // superseded by a newer load
            }
            st.loading = false;
            match result {
                Ok(snap) => loaded(st, *snap, kind, now, fx),
                Err(e) => {
                    st.log(false, e);
                    if kind == LoadKind::Open {
                        super::tabs::open_failed(st, fx);
                    }
                }
            }
        }
        Msg::StatusLoaded { generation, result } => {
            if generation != st.load_gen {
                return;
            }
            match result {
                Ok((unstaged, staged, state)) => {
                    let Some(snap) = st.snap.as_mut() else { return };
                    let was_dirty = snap.is_dirty();
                    snap.unstaged = unstaged;
                    snap.staged = staged;
                    snap.state = state;
                    // Clean <-> dirty changes the graph (the WIP row): reload fully.
                    if was_dirty != snap.is_dirty() {
                        refresh(st, fx);
                    } else {
                        reload_diff(st, fx);
                    }
                }
                Err(e) => {
                    st.log(false, e);
                    // Re-read the open diff anyway, so it doesn't stay stale after a hunk operation.
                    reload_diff(st, fx);
                }
            }
        }
        Msg::SelectWip => {
            st.selection = Selection::Wip;
            st.detail = None;
            let key = st.snap.as_ref().and_then(|s| s.head_branch.clone()).and_then(|b| st.forge.as_ref().and_then(|f| f.branch_issue(&b)));
            super::forge::want_issues(st, key.into_iter().collect(), fx);
        }
        Msg::SelectCommit(oid) => select_commit(st, oid, false, fx),
        Msg::JumpTo(oid) => select_commit(st, oid, true, fx),
        Msg::DetailLoaded { oid, result } => {
            if st.selection != Selection::Commit(oid) {
                return;
            }
            match result {
                Ok(d) => {
                    let keys = st.forge.as_ref().map(|f| f.issue_refs(&d.message)).unwrap_or_default();
                    st.detail = Some(d);
                    super::forge::want_issues(st, keys, fx);
                }
                Err(e) => {
                    st.detail = None;
                    st.log(false, e);
                }
            }
        }
        Msg::OpenDiff(target) => {
            if let Some(workdir) = st.workdir() {
                let generation = next_diff_gen(st);
                fx.push(Effect::LoadDiff { workdir, target, reload: false, generation, previous: None });
            }
        }
        Msg::DiffLoaded { target, reload, generation, result } => {
            if generation != st.diff_gen {
                return; // superseded: another file was opened, the diff closed, or a newer reload sent
            }
            match result {
                Ok(d) => {
                    // The line selection survives only a reload that brings back the same diff.
                    let same = reload && st.diff.as_ref().is_some_and(|(t, old)| *t == target && *old == d);
                    if !same {
                        st.view.diff_lines.clear();
                    }
                    // A different file opens at the top; a reload after a hunk operation keeps its place.
                    if !reload {
                        st.view.diff_scroll_to_row = Some(0);
                        st.view.diff_top_row = 0;
                    }
                    st.diff = Some((target, d));
                    st.diff_shown_gen = generation;
                }
                // A reload that finds no changes left simply closes the diff.
                Err(_) if reload => {
                    st.diff = None;
                    st.view.diff_lines.clear();
                }
                Err(e) => {
                    st.view.diff_lines.clear();
                    st.log(false, e);
                    // The diff still on screen was marked stale by this request: re-read it.
                    reload_diff(st, fx);
                }
            }
        }
        Msg::CloseDiff => close_diff(st),
        Msg::SearchNext => search_next(st, fx),
        Msg::WantStats(oids) => {
            let Some(workdir) = st.workdir() else { return };
            let missing: Vec<Oid> = oids.into_iter().filter(|o| !st.stats.contains_key(o) && !st.stats_pending.contains(o)).collect();
            if !missing.is_empty() {
                st.stats_pending.extend(missing.iter().copied());
                fx.push(Effect::Stats { workdir, oids: missing });
            }
        }
        Msg::StatsLoaded(batch) => {
            for (oid, files, adds, dels) in batch {
                st.stats_pending.remove(&oid);
                st.stats.insert(oid, (files, adds, dels));
            }
        }
        _ => {}
    }
}

/// Loads the repository at `path` into the active tab.
pub(super) fn load(st: &mut AppState, path: PathBuf, kind: LoadKind, fx: &mut Vec<Effect>) {
    st.load_gen += 1;
    st.loading = true;
    fx.push(Effect::LoadRepo { path, opts: st.load_options(), generation: st.load_gen, kind });
}

pub(super) fn refresh(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(path) = st.workdir() {
        load(st, path, LoadKind::Refresh, fx);
    }
}

/// Re-reads only the working-tree status (after staging, discarding…).
pub(super) fn refresh_status(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::LoadStatus { workdir, generation: st.load_gen });
    }
}

pub(super) fn reload_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let (Some(workdir), Some((target, diff))) = (st.workdir(), st.diff.as_ref()) {
        let target = target.clone();
        let previous = diff.highlight.is_some().then(|| diff.clone());
        let generation = next_diff_gen(st);
        fx.push(Effect::LoadDiff { workdir, target, reload: true, generation, previous });
    }
}

/// Starts a new diff request: results still in flight, and hunk operations aimed at the diff
/// on screen, are stale from now on.
fn next_diff_gen(st: &mut AppState) -> u64 {
    st.diff_gen += 1;
    st.diff_gen
}

/// Closes the diff; a load still in flight must not reopen it.
fn close_diff(st: &mut AppState) {
    st.diff = None;
    st.view.diff_lines.clear();
    next_diff_gen(st);
}

fn loaded(st: &mut AppState, snap: Snapshot, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    st.view.base_set = None;
    st.view.highlight = None;
    match kind {
        LoadKind::Open => {
            st.recent.retain(|p| p != &snap.workdir);
            st.recent.insert(0, snap.workdir.clone());
            st.recent.truncate(12);
            fx.push(Effect::SaveRecent(st.recent.clone()));
            fx.push(Effect::Ui(UiEffect::SetTitle(format!("{} - Trellis", snap.name))));
            st.selection = if snap.is_dirty() { Selection::Wip } else { Selection::None };
            st.detail = None;
            close_diff(st);
            st.draft = CommitDraft::default();
            st.undo = Default::default();
            st.view.scroll_to_row = Some(0);
            st.stats.clear();
            st.stats_pending.clear();
        }
        LoadKind::Refresh => {
            if let Selection::Commit(oid) = st.selection {
                let still_there = snap.row_of.contains_key(&oid) || snap.stashes.iter().any(|s| s.oid == oid);
                if !still_there {
                    st.selection = Selection::None;
                    st.detail = None;
                }
            }
        }
    }
    super::forge::detect(st, &snap);
    st.snap = Some(snap);
    if kind == LoadKind::Open {
        fx.push(super::tabs::opened(st));
    }
    super::forge::resolve_pr_heads(st);
    reload_diff(st, fx);
    super::forge::refresh(st, kind == LoadKind::Open, false, now, fx);
}

fn select_commit(st: &mut AppState, oid: Oid, scroll: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.selection = Selection::Commit(oid);
    close_diff(st);
    fx.push(Effect::LoadDetail { workdir, oid });
    if scroll
        && let Some(snap) = &st.snap
        && let Some(&row) = snap.row_of.get(&oid)
    {
        let row = row + snap.graph.wip.is_some() as usize;
        // Leave a few rows of context above the target.
        st.view.scroll_to_row = Some(row.saturating_sub(4));
    }
}

fn search_next(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let q = st.view.search.trim().to_lowercase();
    if q.is_empty() || snap.commits.is_empty() {
        return;
    }
    let n = snap.commits.len();
    let found = (1..=n).map(|k| (st.view.search_cursor + k) % n).find(|&i| commit_matches(&snap.commits[i], &q));
    match found {
        Some(i) => {
            st.view.search_cursor = i;
            let oid = snap.commits[i].oid;
            select_commit(st, oid, true, fx);
        }
        None => {
            let q = st.view.search.trim().to_owned();
            st.log(false, format!("No commits match \"{q}\""));
        }
    }
}

pub fn commit_matches(c: &trellis_core::repo::CommitInfo, q: &str) -> bool {
    c.summary.to_lowercase().contains(q)
        || c.body.to_lowercase().contains(q)
        || c.author.to_lowercase().contains(q)
        || c.oid.to_string().starts_with(q)
}

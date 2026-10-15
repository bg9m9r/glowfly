//! Opening and refreshing the repository, selection, diffs and search.

pub(super) use super::refresh::{load, refresh, refresh_status};
use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::FolderPurpose;
use crate::msg::RepoMsg;
use crate::state::{AppState, CommitDraft, Selection};
use git2::Oid;
use std::time::Instant;
use trellis_core::repo::{DiffTarget, Snapshot};
use trellis_core::settings::DiffMode;

pub(super) fn handle(st: &mut AppState, msg: RepoMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RepoMsg::PickRepo => fx.push(Effect::PickFolder { title: "Open a Git repository".into(), purpose: FolderPurpose::OpenRepo }),
        RepoMsg::InitRepo => {
            fx.push(Effect::PickFolder { title: "Initialize a repository in…".into(), purpose: FolderPurpose::InitRepo })
        }
        RepoMsg::Open(path) => super::tabs::open(st, path, fx),
        RepoMsg::Refresh => {
            // Keys or allowed signers may have changed outside Trellis.
            st.signatures.clear();
            super::refresh::reload(st, fx);
            super::forge::refresh(st, true, false, now, fx);
        }
        RepoMsg::WindowFocused => {
            // Pick up changes made outside the app.
            if st.job.is_none() {
                refresh(st, fx);
            }
            super::recent::check_folders(st, fx);
        }
        RepoMsg::RepoLoaded { generation, kind, result } => {
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
            super::refresh::load_finished(st, fx);
        }
        RepoMsg::SelectWip => {
            st.selection = Selection::Wip;
            st.detail = None;
            let key = st.snap.as_ref().and_then(|s| s.head_branch.clone()).and_then(|b| st.forge.as_ref().and_then(|f| f.branch_issue(&b)));
            super::forge::want_issues(st, key.into_iter().collect(), fx);
        }
        RepoMsg::SelectCommit(oid) => select_commit(st, oid, false, fx),
        RepoMsg::JumpTo(oid) => select_commit(st, oid, true, fx),
        RepoMsg::DetailLoaded { oid, result } => {
            if st.selection != Selection::Commit(oid) {
                return;
            }
            match result {
                Ok(d) => {
                    let keys = st.forge.as_ref().map(|f| f.issue_refs(&d.message)).unwrap_or_default();
                    st.detail = Some(d);
                    super::signatures::want(st, oid, fx);
                    super::forge::want_issues(st, keys, fx);
                }
                Err(e) => {
                    st.detail = None;
                    st.log(false, e);
                }
            }
        }
        RepoMsg::OpenDiff(target) => {
            super::history::close(st);
            open_diff(st, target, fx);
        }
        RepoMsg::DiffLoaded { target, reload, generation, result } => {
            if generation != st.diff_gen {
                return; // superseded: another file was opened, the diff closed, or a newer reload sent
            }
            // Any accepted result ends a pending step, so a superseding reload can't block it.
            let landing = st.view.diff_landing.take();
            match result {
                Ok(mut d) => {
                    let landing = landing.filter(|(t, _)| !reload && *t == target).map(|(_, l)| l);
                    // A stepped-to file with nothing to land on is passed over, never shown.
                    if let Some(l) = landing
                        && super::diff::skip_past(st, &target, &d, l, fx)
                    {
                        return;
                    }
                    // The line selection survives only a reload that brings back the same diff.
                    // Only the text counts: a provisional or pending highlight doesn't make it differ.
                    let same =
                        reload && st.diff.as_ref().is_some_and(|(t, old)| *t == target && old.binary == d.binary && old.hunks == d.hunks);
                    if !same {
                        st.view.diff_lines.clear();
                    }
                    // Folds opened in another file don't apply to this one.
                    if !matches!(&st.diff, Some((t, _)) if *t == target) {
                        st.view.diff_expanded.clear();
                    }
                    // A different file opens at the top (or its first change block, or the hunk kept
                    // through a view mode switch, or the change a step lands on); a reload after a
                    // hunk operation keeps its place, but its change rows may have moved.
                    if !reload {
                        let hunk = st.view.diff_open_hunk.take();
                        let row = match landing {
                            Some(l) => super::diff::landing_row(&d, &st.settings, l, &st.view.diff_expanded),
                            None => super::diff::opening_row(&d, &st.settings, hunk, &st.view.diff_expanded),
                        };
                        st.view.diff_scroll_to_row = Some(row);
                        st.view.diff_top_row = row;
                        st.view.diff_cursor = landing.map(|_| row);
                    } else {
                        st.view.diff_cursor = None;
                    }
                    if reload {
                        super::highlight::keep_finished(st.diff.as_ref(), &target, &mut d);
                    }
                    st.diff = Some((target, std::sync::Arc::new(d)));
                    st.diff_shown_gen = generation;
                    super::highlight::schedule(st, fx);
                }
                // A reload that finds no changes left simply closes the diff.
                Err(_) if reload => {
                    st.diff = None;
                    st.view.diff_lines.clear();
                    st.view.diff_expanded.clear();
                    st.view.diff_open_hunk = None;
                }
                Err(e) => {
                    st.view.diff_lines.clear();
                    st.view.diff_open_hunk = None;
                    st.log(false, e);
                    // The diff still on screen was marked stale by this request: re-read it. Not
                    // `reload_diff`, which would ask again for a file history entry that failed.
                    reload_shown_diff(st, fx);
                }
            }
        }
        RepoMsg::CloseDiff => close_diff(st),
        RepoMsg::SearchNext => search_next(st, fx),
        RepoMsg::WantStats(oids) => {
            let Some(workdir) = st.workdir() else { return };
            let missing: Vec<Oid> = oids.into_iter().filter(|o| !st.stats.contains_key(o) && !st.stats_pending.contains(o)).collect();
            if !missing.is_empty() {
                st.stats_pending.extend(missing.iter().copied());
                fx.push(Effect::Stats { workdir, oids: missing });
            }
        }
        RepoMsg::StatsDropped(oids) => {
            for oid in oids {
                st.stats_pending.remove(&oid);
            }
        }
        RepoMsg::StatsLoaded(batch) => {
            for (oid, files, adds, dels) in batch {
                st.stats_pending.remove(&oid);
                st.stats.insert(oid, (files, adds, dels));
            }
        }
        RepoMsg::WantSignature(oid) => super::signatures::want(st, oid, fx),
        RepoMsg::CheckSignatureAgain(oid) => super::signatures::again(st, oid, fx),
        RepoMsg::SignatureChecked { oid, generation, result } => super::signatures::checked(st, oid, generation, result),
    }
}

pub(super) fn reload_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    if open_missing_history_diff(st, fx) {
        return;
    }
    reload_shown_diff(st, fx);
}

/// Opens the diff on screen again, as if newly opened (at the top, or at its first change
/// block in the whole-file view; no line selection), e.g. after the diff options changed.
/// Does nothing when no diff is open.
pub(super) fn reopen_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    if open_missing_history_diff(st, fx) {
        return;
    }
    request_diff(st, fx, false);
}

/// The file history's selected entry may not be on screen yet (its request still in flight
/// or dropped with a tab switch): ask for it, as a request for the previous entry would
/// supersede it. Returns whether it did.
fn open_missing_history_diff(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    match super::history::missing_diff(st) {
        Some(target) => {
            open_diff(st, target, fx);
            true
        }
        None => false,
    }
}

/// Re-reads the diff on screen, whatever the file history has selected.
fn reload_shown_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    request_diff(st, fx, true);
}

fn request_diff(st: &mut AppState, fx: &mut Vec<Effect>, reload: bool) {
    if let (Some(workdir), Some((target, diff))) = (st.workdir(), st.diff.as_ref()) {
        let target = target.clone();
        let highlighted = diff.highlight.is_some() || diff.whole.as_ref().is_some_and(|w| w.diff.highlight.is_some());
        let previous = highlighted.then(|| std::sync::Arc::clone(diff));
        let generation = next_diff_gen(st);
        let ignore_whitespace = st.settings.diff_ignore_whitespace;
        let whole_file = st.settings.diff_mode == DiffMode::WholeFile;
        fx.push(Effect::LoadDiff { workdir, target, reload, generation, previous, ignore_whitespace, whole_file });
    }
}

/// Starts a new diff request: results still in flight, and hunk operations aimed at the diff
/// on screen, are stale from now on.
fn next_diff_gen(st: &mut AppState) -> u64 {
    st.diff_gen += 1;
    st.diff_gen
}

/// Opens `target` in the diff viewer (closing a conflict view).
pub(super) fn open_diff(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    super::conflict::close(st);
    // A different file opens: a hunk kept through a view mode switch or a pending step is moot.
    st.view.diff_open_hunk = None;
    st.view.diff_landing = None;
    st.view.diff_cursor = None;
    if let Some(workdir) = st.workdir() {
        let generation = next_diff_gen(st);
        let ignore_whitespace = st.settings.diff_ignore_whitespace;
        let whole_file = st.settings.diff_mode == DiffMode::WholeFile;
        fx.push(Effect::LoadDiff { workdir, target, reload: false, generation, previous: None, ignore_whitespace, whole_file });
    }
}

/// Closes the diff; a load still in flight must not reopen it.
pub(super) fn close_diff(st: &mut AppState) {
    st.diff = None;
    st.view.diff_lines.clear();
    st.view.diff_expanded.clear();
    st.view.diff_open_hunk = None;
    st.view.diff_landing = None;
    st.view.diff_cursor = None;
    next_diff_gen(st);
}

fn loaded(st: &mut AppState, snap: Snapshot, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    if kind == LoadKind::Open && super::tabs::open_elsewhere(st, &snap, fx) {
        return;
    }
    st.view.base_set = None;
    st.view.highlight = None;
    match kind {
        LoadKind::Open => {
            st.recent.retain(|p| p != &snap.workdir);
            st.recent.insert(0, snap.workdir.clone());
            st.recent.truncate(12);
            fx.push(Effect::SaveRecent(st.recent.clone()));
            st.missing_repos.retain(|p| !crate::state::tabs::same_path(p, &snap.workdir));
            fx.push(Effect::Ui(UiEffect::SetTitle(format!("{} - Trellis", snap.name))));
            st.selection = if snap.is_dirty() { Selection::Wip } else { Selection::None };
            st.detail = None;
            close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::rebase::close(st);
            st.draft = CommitDraft::default();
            st.undo = Default::default();
            st.view.scroll_to_row = Some(0);
            st.stats.clear();
            st.stats_pending.clear();
            st.signatures.clear();
        }
        LoadKind::Refresh => {
            if let Selection::Commit(oid) = st.selection {
                let still_there = snap.row_of.contains_key(&oid) || snap.stashes.iter().any(|s| s.oid == oid);
                if !still_there {
                    st.selection = Selection::None;
                    st.detail = None;
                }
            }
            if matches!(st.selection, Selection::Commits(_)) {
                super::selection::refreshed(st, &snap, fx);
            }
            super::rebase::refreshed(st, &snap);
            super::shallow::refreshed(st, &snap, fx);
        }
    }
    super::forge::detect(st, &snap);
    let reload = super::graph_filter::loaded(st, &snap, kind, fx);
    st.snap = Some(snap);
    super::trust::loaded(st, kind, fx);
    super::gitflow::snapshot_changed(st);
    super::commit::apply_template(st);
    super::watch::sync(st, fx);
    super::terminal::sync(st, fx);
    if kind == LoadKind::Open {
        fx.push(super::tabs::opened(st));
    } else {
        super::conflict::status_changed(st, now);
    }
    super::forge::resolve_pr_heads(st);
    reload_diff(st, fx);
    super::forge::refresh(st, kind == LoadKind::Open, false, now, fx);
    if reload {
        super::refresh::reload_after(st, fx);
    }
}

pub(super) fn select_commit(st: &mut AppState, oid: Oid, scroll: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.selection = Selection::Commit(oid);
    close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
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

/// Whether the commit's summary, body or author contains `q`, or its id starts with it. `q` is
/// lowercase. Runs for every row the graph dims and over the history on Find next, so it
/// doesn't allocate.
pub fn commit_matches(c: &trellis_core::repo::CommitInfo, q: &str) -> bool {
    use trellis_core::matching::{contains_lower, hex_starts_with};
    contains_lower(&c.summary, q) || contains_lower(&c.body, q) || contains_lower(&c.author, q) || hex_starts_with(c.oid.as_bytes(), q)
}

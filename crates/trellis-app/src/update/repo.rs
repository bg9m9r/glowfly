//! Opening and refreshing the repository, selection and diffs.

pub(super) use super::refresh::{load, refresh, refresh_status};
use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::FolderPurpose;
use crate::msg::RepoMsg;
use crate::state::{AppState, CommitDraft, Selection};
use git2::Oid;
use std::time::Instant;
use trellis_core::diff_rows::row_of_file_line;
use trellis_core::repo::{DiffTarget, Snapshot};
use trellis_core::settings::DiffMode;

pub(super) fn handle(st: &mut AppState, msg: RepoMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RepoMsg::PickRepo => fx.push(Effect::PickFolder { title: "Open a Git repository".into(), purpose: FolderPurpose::OpenRepo }),
        RepoMsg::InitRepo => super::init::open(st, fx),
        RepoMsg::Open(path) => super::tabs::open(st, path, fx),
        RepoMsg::Refresh => {
            // Keys or allowed signers may have changed outside Trellis.
            st.signatures.clear();
            st.conflict_checks.invalidate();
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
                Ok(snap) => {
                    st.refresh.more_retries = 0;
                    loaded(st, *snap, kind, now, fx)
                }
                Err(e) => {
                    // The snapshot is unchanged, so a load for more is still wanted.
                    if st.loading_more() {
                        super::load_more::failed(st, e, now);
                    } else {
                        st.log(false, e);
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
            super::refresh::load_finished(st, fx);
        }
        RepoMsg::SelectWip => select_wip(st, fx),
        RepoMsg::ShowInProgress => super::op_progress::show(st, fx),
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
        RepoMsg::OpenDiff(target) => open_file_diff(st, target, fx),
        RepoMsg::DiffLoaded { target, reload, generation, result, lfs } => {
            if generation != st.diff_gen {
                return; // superseded: another file was opened, the diff closed, or a newer reload sent
            }
            st.view.diff_opening = None;
            // Any accepted result ends a pending step. A refresh while it loads asks for the step's
            // destination again (`diff::renew_step`) rather than reloading the old file, so a
            // superseding reload can't block it.
            let landing = st.view.diff_landing.take();
            let line = st.view.diff_open_line.take().filter(|(t, ..)| !reload && *t == target);
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
                            None => {
                                let (s, ex) = (&st.settings, &st.view.diff_expanded);
                                // A file opened from a code comment lands on its line.
                                line.and_then(|(_, side, n)| row_of_file_line(&d, s.diff_mode, s.diff_layout, ex, side, n))
                                    .unwrap_or_else(|| super::diff::opening_row(&d, s, hunk, ex))
                            }
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
                    // A step landed: its file list shows the file once.
                    if landing.is_some() {
                        st.view.file_reveal = Some((target.clone(), generation));
                    }
                    super::lfs::diff_loaded(st, &target, lfs);
                    st.diff = Some((target, std::sync::Arc::new(d)));
                    st.diff_shown_gen = generation;
                    super::highlight::schedule(st, fx);
                    super::diff_gaps::diff_shown(st, fx);
                    super::diff_preview::diff_shown(st, fx);
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
        RepoMsg::WantStats(oids) => super::line_counts::want_totals(st, oids, fx),
        RepoMsg::StatsDropped(oids) => super::line_counts::totals_dropped(st, oids),
        RepoMsg::StatsLoaded(batch) => super::line_counts::totals_loaded(st, batch),
        RepoMsg::CommitLineCountsLoaded { oid, result } => super::line_counts::commit_loaded(st, oid, result),
        RepoMsg::WipLineCountsLoaded { generation, result } => super::line_counts::wip_loaded(st, generation, result, fx),
        RepoMsg::WantSignature(oid) => super::signatures::want(st, oid, fx),
        RepoMsg::CheckSignatureAgain(oid) => super::signatures::again(st, oid, fx),
        RepoMsg::SignatureChecked { oid, generation, result } => super::signatures::checked(st, oid, generation, result),
    }
}

pub(super) fn reload_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    if open_missing_history_diff(st, fx) || super::diff::renew_step(st, fx) || super::wip_list::renew_open(st, fx) {
        return;
    }
    reload_shown_diff(st, fx);
}

/// Opens the diff on screen again, as if newly opened (at the top, or at its first change
/// block in the whole-file view; no line selection), e.g. after the diff options changed.
/// Does nothing when no diff is open.
pub(super) fn reopen_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    if open_missing_history_diff(st, fx) || super::diff::renew_step(st, fx) || super::wip_list::renew_open(st, fx) {
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

/// Opens a file's diff from a list (closing file history and blame, and the pull request panel
/// unless it is one of its files).
pub(super) fn open_file_diff(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    super::history::close(st);
    super::blame::close(st);
    super::pull_request::keep_for(st, &target);
    super::issue::close(st);
    open_diff(st, target, fx);
}

/// Opens `target` in the diff viewer (closing a conflict view).
pub(super) fn open_diff(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    super::conflict::close(st);
    // A different file opens: a hunk kept through a view mode switch or a pending step is moot.
    st.view.diff_open_hunk = None;
    st.view.diff_landing = None;
    st.view.file_reveal = None;
    st.view.diff_open_line = None;
    st.view.diff_cursor = None;
    if let Some(workdir) = st.workdir() {
        let generation = next_diff_gen(st);
        let ignore_whitespace = st.settings.diff_ignore_whitespace;
        let whole_file = st.settings.diff_mode == DiffMode::WholeFile;
        st.view.diff_opening = Some(target.clone());
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
    st.view.file_reveal = None;
    st.view.diff_open_line = None;
    st.view.diff_cursor = None;
    st.view.diff_opening = None;
    st.diff_preview = None;
    next_diff_gen(st);
}

fn loaded(st: &mut AppState, mut snap: Snapshot, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    let state_before = st.snap.as_ref().filter(|_| kind == LoadKind::Refresh).map(|s| s.state);
    st.known.learn_snapshot(&snap);
    if kind == LoadKind::Open && super::tabs::open_elsewhere(st, &snap, fx) {
        super::deep_link_open::ready(st, now, fx);
        return;
    }
    st.view.base_set = None;
    st.view.highlight = None;
    match kind {
        LoadKind::Open => {
            // Any spelling of this folder (a short 8.3 name, a link, another case where the
            // volume ignores it) gives way to libgit2's.
            let recent = std::mem::take(&mut st.recent);
            st.recent = recent.into_iter().filter(|p| !st.names_repo(&snap, p)).collect();
            st.recent.insert(0, snap.workdir.clone());
            st.recent.truncate(12);
            fx.push(Effect::SaveRecent(st.recent.clone()));
            let missing = std::mem::take(&mut st.missing_repos);
            st.missing_repos = missing.into_iter().filter(|p| !st.names_repo(&snap, p)).collect();
            let label = st.tabs.get(st.active_tab).and_then(|t| t.label.as_deref());
            fx.push(Effect::Ui(UiEffect::SetTitle(format!("{} - Trellis", label.unwrap_or(&snap.name)))));
            st.selection = if snap.is_dirty() { Selection::Wip } else { Selection::None };
            st.detail = None;
            close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            super::pull_request::close(st);
            super::issue::close(st);
            super::rebase::close(st);
            super::journal::close(st);
            st.gitflow_panel = false;
            st.draft = CommitDraft::default();
            st.undo = Default::default();
            st.view.scroll_to_row = Some(0);
            st.stats.clear();
            st.stats_pending.clear();
            st.line_counts.clear();
            st.signatures.clear();
            st.conflict_checks.clear();
        }
        LoadKind::Refresh => {
            super::history::head_moved(st, &snap, fx);
            super::blame::head_moved(st, &snap, fx);
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
            super::compare_wip::refreshed(st, &snap, fx);
            super::rebase::refreshed(st, &snap);
            super::shallow::refreshed(st, &snap, fx);
        }
    }
    super::forge::detect(st, &snap);
    let reload = super::graph_filter::loaded(st, &snap, kind, fx);
    super::notify::loaded(st, &snap, kind, now, fx);
    // After the filter, which adopts the repository's keys: the pins are found under its own.
    super::branch_colour::apply(&st.settings, &mut snap);
    st.snap = Some(snap);
    super::line_counts::status_changed(st, fx);
    super::pick::state_changed(st, state_before);
    super::auto_stash::seen(st, true, now, fx);
    super::search::snapshot_changed(st, fx);
    super::trust::loaded(st, kind, fx);
    super::custom_commands::loaded(st, fx);
    super::submodule_batch::opened(st, kind, now, fx);
    super::conflict_check::want(st, fx);
    super::worktree_counts::want(st, fx);
    super::gitflow::snapshot_changed(st);
    super::commit::prefill(st);
    super::watch::sync(st, fx);
    super::terminal::sync(st, fx);
    if kind == LoadKind::Open {
        fx.push(super::tabs::opened(st));
        super::conflict_files::want(st, fx);
    } else {
        super::conflict::status_changed(st, now, fx);
    }
    super::forge::resolve_pr_heads(st);
    if kind == LoadKind::Refresh {
        super::pull_request::refreshed(st, fx);
        super::journal::refreshed(st, fx);
    }
    reload_diff(st, fx);
    super::forge::refresh(st, kind == LoadKind::Open, false, now, fx);
    super::push_pr::loaded(st, fx);
    super::onboarding::loaded(st, kind);
    let slow = st.snap.as_ref().and_then(|s| s.slow_status.clone());
    super::status_tip::check(st, slow.as_ref(), now, fx);
    super::deep_link_open::ready(st, now, fx);
    if reload {
        super::refresh::reload_after(st, fx);
    }
}

/// Selects the WIP row, asking for the issue the checked-out branch names.
pub(super) fn select_wip(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.selection = Selection::Wip;
    st.detail = None;
    let key = st.snap.as_ref().and_then(|s| s.head_branch.clone()).and_then(|b| st.forge.as_ref().and_then(|f| f.branch_issue(&b)));
    super::forge::want_issues(st, key.into_iter().collect(), fx);
}

pub(super) fn select_commit(st: &mut AppState, oid: Oid, scroll: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.selection = Selection::Commit(oid);
    close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    st.gitflow_panel = false;
    super::journal::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    fx.push(Effect::LoadDetail { workdir, oid });
    super::line_counts::want_commit(st, oid, fx);
    if scroll && let Some(row) = super::search::reveal(st, oid) {
        // Leave a few rows of context above the target.
        st.view.scroll_to_row = Some(row.saturating_sub(4));
    }
}

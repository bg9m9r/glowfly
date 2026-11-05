//! Opening and refreshing the repository, selection and diffs.

pub(super) use super::refresh::{load, refresh, refresh_status};
use crate::effect::{CommitEffect, Effect, OsEffect};
use crate::msg::{FolderPurpose, RepoMsg};
use crate::state::error_words::typed;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: RepoMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RepoMsg::PickRepo => {
            fx.push(Effect::Os(OsEffect::PickFolder { title: "Open a Git repository".into(), purpose: FolderPurpose::OpenRepo }))
        }
        RepoMsg::InitRepo => super::init::open(st, fx),
        RepoMsg::Open(path) => super::tabs::open(st, path, fx),
        RepoMsg::Refresh => {
            // Keys or allowed signers may have changed outside Glowfly.
            st.session.reads.signatures.clear();
            st.session.reads.conflict_checks.invalidate();
            super::refresh::reload(st, fx);
            super::forge::refresh(st, true, false, now, fx);
        }
        RepoMsg::WindowFocused => {
            // Pick up changes made outside the app, other worktrees' included at once.
            st.refresh.recount.forget();
            if st.job.is_none() {
                refresh(st, fx);
            }
            super::recent::check_folders(st, fx);
            super::profile_rules::want_identity(st, fx);
        }
        RepoMsg::RepoLoaded { generation, kind, result } => {
            let t = st.settings.lang();
            super::repo_loaded::finished(st, generation, kind, result.map_err(|e| typed::read(t, &e)), now, fx)
        }
        RepoMsg::SelectWip => select_wip(st, fx),
        RepoMsg::ShowInProgress => super::op_progress::show(st, fx),
        RepoMsg::SelectCommit(oid) => select_commit(st, oid, false, fx),
        RepoMsg::JumpTo(oid) => select_commit(st, oid, true, fx),
        RepoMsg::DetailLoaded { oid, result } => {
            if st.session.selection != Selection::Commit(oid) {
                return;
            }
            match result {
                Ok(d) => {
                    let keys = st.session.forge.as_ref().map(|f| f.issue_refs(&d.message)).unwrap_or_default();
                    st.session.detail = Some(d);
                    super::signatures::want(st, oid, fx);
                    super::forge::want_issues(st, keys, fx);
                }
                Err(e) => {
                    st.session.detail = None;
                    st.log(false, typed::read(st.settings.lang(), &e));
                }
            }
        }
        RepoMsg::OpenDiff(target) => open_file_diff(st, target, fx),
        RepoMsg::DiffLoaded { target, reload, generation, result, lfs, raw } => {
            let t = st.settings.lang();
            let result = result.map_err(|e| typed::read(t, &e));
            super::diff_loaded::loaded(st, target, reload, generation, result, lfs, raw, fx)
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

pub fn reload_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
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
pub(super) fn reload_shown_diff(st: &mut AppState, fx: &mut Vec<Effect>) {
    request_diff(st, fx, true);
}

fn request_diff(st: &mut AppState, fx: &mut Vec<Effect>, reload: bool) {
    if let (Some(workdir), Some((target, diff))) = (st.workdir(), st.session.diff.view.as_ref()) {
        let target = target.clone();
        let highlighted = diff.highlight.is_some() || diff.whole.as_ref().is_some_and(|w| w.diff.highlight.is_some());
        let previous = highlighted.then(|| std::sync::Arc::clone(diff));
        let generation = st.session.diff.next();
        fx.push(super::diff::load_effect(st, workdir, target, reload, generation, previous));
    }
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

/// Opens `target` in the diff viewer (closing a conflict view), with no lines selected.
pub(super) fn open_diff(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    super::conflict::close(st);
    // Nothing is selected by default: lines or text picked in the diff on screen don't carry
    // over, not even while the new one loads.
    st.clear_diff_selection();
    // A different file opens: a hunk kept through a view mode switch or a pending step is moot.
    st.cues.diff_open_hunk = None;
    st.cues.diff_landing = None;
    st.cues.file_reveal = None;
    st.cues.diff_open_line = None;
    st.cues.diff_cursor = None;
    if let Some(workdir) = st.workdir() {
        let generation = st.session.diff.next();
        st.cues.diff_opening = Some(target.clone());
        super::wip_list::opened(st, &target);
        fx.push(super::diff::load_effect(st, workdir, target, false, generation, None));
    }
}

/// Closes the diff; a load still in flight must not reopen it.
pub(super) fn close_diff(st: &mut AppState) {
    st.session.diff.view = None;
    st.clear_diff_selection();
    st.session.tab_view.diff_expanded.clear();
    st.cues.diff_open_hunk = None;
    st.cues.diff_landing = None;
    st.cues.file_reveal = None;
    st.cues.diff_open_line = None;
    st.cues.diff_cursor = None;
    st.cues.diff_opening = None;
    st.session.diff_preview = None;
    st.session.diff.next();
}

/// Selects the WIP row as chosen (`wip_lit`), asking for the issue the checked-out branch names.
pub(super) fn select_wip(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.session.selection = Selection::Wip;
    st.session.wip_lit = true;
    st.session.detail = None;
    let key = st.snap.as_ref().and_then(|s| s.head_branch.clone()).and_then(|b| st.session.forge.as_ref().and_then(|f| f.branch_issue(&b)));
    super::forge::want_issues(st, key.into_iter().collect(), fx);
}

pub(super) fn select_commit(st: &mut AppState, oid: Oid, scroll: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.session.selection = Selection::Commit(oid);
    close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    st.session.gitflow_panel = false;
    super::journal::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    fx.push(Effect::Commit(CommitEffect::LoadDetail { workdir, oid }));
    super::line_counts::want_commit(st, oid, fx);
    if scroll && let Some(row) = super::search::reveal(st, oid) {
        // Leave a few rows of context above the target.
        st.view.scroll_to_row = Some(row.saturating_sub(4));
    }
}

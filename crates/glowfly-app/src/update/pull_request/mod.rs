//! The pull request panel: opening it for a listed pull request, following refreshes of the refs
//! and of the service's list, switching tabs, and closing it. Where its files and commits come
//! from (and the head ref fetch) is in source.rs, the Commits tab's selection in commits.rs. Its
//! diffs are the shared `st.diff`, opened through `RepoMsg::OpenDiff`; its page and actions on
//! the service are in calls.rs, its Checks section in checks.rs, its code comment threads in
//! threads.rs (placed on the local version's lines by line_map.rs when the files aren't of the
//! service's head), and a new code comment on diff lines in line_comment.rs.

use crate::effect::Effect;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Files, PrTab, PrView};
use crate::state::pull_request_line_map::LineMaps;
use crate::state::pull_request_source::HeadFetch;
use calls::load;
use glowfly_core::repo::DiffTarget;
use source::{request, retip, shows_diff};
use std::time::Instant;

pub(super) mod calls;
#[cfg(test)]
mod calls_tests;
mod checks;
#[cfg(test)]
mod checks_tests;
mod commits;
#[cfg(test)]
mod commits_tests;
mod line_comment;
#[cfg(test)]
mod line_comment_reply_tests;
#[cfg(test)]
mod line_comment_tests;
mod line_map;
#[cfg(test)]
mod line_map_tests;
#[cfg(test)]
mod merge_tests;
mod source;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
pub(in crate::update) mod tests;
mod threads;
#[cfg(test)]
mod threads_tests;

pub(super) fn handle(st: &mut AppState, msg: PullRequestMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        PullRequestMsg::Open(id) => {
            let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
            let Some(pr) = fs.prs.iter().find(|p| p.id == id) else { return };
            if st.pull_request.as_ref().is_some_and(|v| v.pr.id == id) {
                return;
            }
            let view = PrView::new(pr.clone(), fs, snap);
            super::repo::close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            super::issue::close(st);
            st.pull_request = Some(view);
            request(st, fx);
            load(st, fx);
            checks::load(st, fx);
        }
        PullRequestMsg::Close => close(st),
        PullRequestMsg::FilesLoaded { id, generation, result } => source::files_loaded(st, id, generation, result, fx),
        PullRequestMsg::Tab(t) => {
            if let Some(v) = st.pull_request.as_mut() {
                v.tab = t;
            }
        }
        PullRequestMsg::SelectCommit(oid) => commits::select(st, oid, fx),
        PullRequestMsg::FetchHead => source::fetch_clicked(st, fx),
        PullRequestMsg::HeadFetched { id, head, result } => source::head_fetched(st, id, head, result, fx),
        PullRequestMsg::LinesMapped { id, generation, result } => line_map::mapped(st, id, generation, result),
        PullRequestMsg::Reload => load(st, fx),
        PullRequestMsg::ReloadFiles => request(st, fx),
        PullRequestMsg::ToggleChecks => checks::toggle(st),
        PullRequestMsg::ReloadChecks => checks::load(st, fx),
        PullRequestMsg::PostComment => calls::post_comment(st, fx),
        PullRequestMsg::SubmitReview => calls::submit_review(st, fx),
        PullRequestMsg::Merge => calls::merge(st, fx),
        PullRequestMsg::Reply(id) => threads::reply(st, id, fx),
        PullRequestMsg::Resolve { thread, resolve } => threads::resolve(st, thread, resolve, fx),
        PullRequestMsg::ShowResolved(show) => threads::show_resolved(st, show),
        PullRequestMsg::ShowOutdated(show) => threads::show_outdated(st, show),
        PullRequestMsg::ToggleThread(id) => threads::toggle(st, id),
        PullRequestMsg::FocusThread(id) => threads::focus(st, id),
        PullRequestMsg::GoToThread(id) => threads::go_to_thread(st, id, fx),
        PullRequestMsg::LineComment(m) => line_comment::handle(st, m, now, fx),
        PullRequestMsg::Replied { workdir, target, id, generation, reply } => {
            calls::replied(st, &workdir, &target, id, generation, *reply, now, fx)
        }
    }
}

/// Closes the panel, and the diff when it is one of its files; a list or page still in flight
/// must not reopen it.
pub(super) fn close(st: &mut AppState) {
    let shown = shows_diff(st);
    if st.pull_request.take().is_some() {
        st.pull_request_gen += 1;
        st.pull_request_page_gen += 1;
        st.pull_request_checks_gen += 1;
        st.pull_request_lines_gen += 1;
        if shown {
            super::repo::close_diff(st);
        }
    }
}

/// A diff is opening: the panel stays only when it is one of its files (shown in Files
/// changed) or of one of its commits (shown in Commits).
pub(super) fn keep_for(st: &mut AppState, target: &DiffTarget) {
    match st.pull_request.as_mut() {
        Some(v) if v.shows(target) => v.tab = if matches!(target, DiffTarget::Commit(..)) { PrTab::Commits } else { PrTab::Files },
        _ => close(st),
    }
}

/// A full load stored a new snapshot: when the remote-tracking tips moved, the files load
/// again (and a diff of the old list closes).
pub(super) fn refreshed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let remote = st.forge.as_ref().filter(|f| f.host.is_some()).map(|f| f.remote.clone());
    let Some(view) = st.pull_request.as_mut() else { return };
    if let Some(r) = remote {
        view.remote = r; // follows a renamed remote
    }
    retip(st, fx, false);
}

/// The service's list was replaced: the panel's copy follows it, or notes that it is gone. A
/// newly listed head may be here already, so the files are compared again (and its head ref
/// may be fetched again). When the listed head moved past the loaded page's, the page loads again (its mergeability
/// and head belong to the old one); the checks load again then too, and when the listed
/// summary of them changed (a check finished, say).
pub(super) fn prs_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = st.pull_request.as_mut() else { return };
    let (mut stale, mut checks_stale, mut head_changed) = (false, false, false);
    match st.forge.as_ref().and_then(|f| f.prs.iter().find(|p| p.id == view.pr.id)) {
        Some(pr) => {
            let summary_changed = view.pr.checks != pr.checks;
            head_changed = view.pr.head != pr.head;
            let head_moved = pr.head.is_some() && view.checks.loaded().is_some_and(|l| !l.sha.is_empty() && !pr.head_matches(&l.sha));
            view.pr = pr.clone();
            view.listed = true;
            stale = view.loaded().is_some_and(|p| pr.head.is_some() && !pr.head_matches(&p.detail.head_sha));
            checks_stale = summary_changed || head_moved;
        }
        None => view.listed = false,
    }
    if head_changed {
        view.head_fetch = HeadFetch::Idle;
        source::rekey(st, fx, false);
    }
    if stale {
        load(st, fx);
    }
    if stale || checks_stale {
        checks::load(st, fx);
    }
}

/// Once a minute while a check is still running: reads the open panel's checks again.
pub(super) fn checks_tick(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    checks::tick(st, now, fx);
}

/// After a tab switch: a list dropped while the tab was parked, or one whose tips moved, is
/// asked for again (a head fetch whose result was dropped starts again once nothing is found),
/// the line maps are asked for again while they were loading, and the page and the checks
/// load again in the background. An action's reply that came
/// while the tab was parked was only logged, so the panel no longer waits for it.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (mut again, mut lines_again) = (false, false);
    if let Some(v) = st.pull_request.as_mut() {
        v.busy = None;
        lines_again = matches!(v.lines, LineMaps::Loading(_));
        again = v.files == Files::Loading || v.head_fetch == HeadFetch::Running;
        if v.head_fetch == HeadFetch::Running {
            v.head_fetch = HeadFetch::Idle;
        }
    }
    retip(st, fx, again);
    line_map::sync(st, fx, lines_again);
    load(st, fx);
    checks::load(st, fx);
}

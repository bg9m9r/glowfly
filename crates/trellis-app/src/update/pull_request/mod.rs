//! The pull request panel: opening it for a listed pull request, loading the files it changes
//! from the local remote-tracking branches, following refreshes of the refs and of the
//! service's list, switching tabs, and closing it. Its files' diffs are the shared `st.diff`,
//! opened through `RepoMsg::OpenDiff`; its page and actions on the service are in
//! calls.rs, and its code comment threads in threads.rs.

use crate::effect::Effect;
use crate::pull_request::{FilesJob, PrEffect, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::{Files, PrTab, PrView, Tips};
use calls::load;
use std::time::Instant;
use trellis_core::repo::DiffTarget;

pub(super) mod calls;
#[cfg(test)]
mod calls_tests;
#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod tests;
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
        }
        PullRequestMsg::Close => close(st),
        PullRequestMsg::FilesLoaded { id, tips, generation, result } => {
            if generation != st.pull_request_gen {
                return; // superseded: closed, reopened, the tips moved, or parked with its tab
            }
            let Some(view) = st.pull_request.as_mut().filter(|v| v.pr.id == id && v.tips.pair() == Some(tips)) else { return };
            match result {
                Ok(Some(c)) => view.files = Files::Loaded(c),
                Ok(None) => view.files = Files::Unrelated,
                Err(e) => {
                    view.files = Files::Failed(e.clone());
                    st.log(false, e);
                }
            }
        }
        PullRequestMsg::Tab(t) => {
            if let Some(v) = st.pull_request.as_mut() {
                v.tab = t;
            }
        }
        PullRequestMsg::Reload => load(st, fx),
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
        PullRequestMsg::Replied { workdir, target, id, generation, reply } => {
            calls::replied(st, &workdir, &target, id, generation, *reply, now, fx)
        }
    }
}

/// Asks for the file list of the open view's tips; older results are ignored from now on.
/// Without both tips there is nothing to compare.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.pull_request_gen += 1;
    let generation = st.pull_request_gen;
    let workdir = st.workdir();
    let Some(view) = st.pull_request.as_mut() else { return };
    match (view.tips.pair(), workdir) {
        (Some(tips), Some(workdir)) => {
            view.files = Files::Loading;
            fx.push(Effect::PullRequest(PrEffect::Files(FilesJob { workdir, id: view.pr.id, tips, generation })));
        }
        _ => view.files = Files::Missing,
    }
}

/// Whether the diff on screen is one of the open pull request's files.
fn shows_diff(st: &AppState) -> bool {
    match (&st.pull_request, &st.diff) {
        (Some(v), Some((t, _))) => v.shows(t),
        _ => false,
    }
}

/// Closes the panel, and the diff when it is one of its files; a list or page still in flight
/// must not reopen it.
pub(super) fn close(st: &mut AppState) {
    let shown = shows_diff(st);
    if st.pull_request.take().is_some() {
        st.pull_request_gen += 1;
        st.pull_request_page_gen += 1;
        if shown {
            super::repo::close_diff(st);
        }
    }
}

/// A diff is opening: the panel stays only when it is one of its files, and then shows them.
pub(super) fn keep_for(st: &mut AppState, target: &DiffTarget) {
    match st.pull_request.as_mut() {
        Some(v) if v.shows(target) => v.tab = PrTab::Files,
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

/// The service's list was replaced: the panel's copy follows it, or notes that it is gone.
/// When the listed head moved past the loaded page's, the page loads again (its mergeability
/// and head belong to the old one).
pub(super) fn prs_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = st.pull_request.as_mut() else { return };
    let mut stale = false;
    match st.forge.as_ref().and_then(|f| f.prs.iter().find(|p| p.id == view.pr.id)) {
        Some(pr) => {
            view.pr = pr.clone();
            view.listed = true;
            stale = view.loaded().is_some_and(|p| pr.head.is_some() && !pr.head_matches(&p.detail.head_sha));
        }
        None => view.listed = false,
    }
    if stale {
        load(st, fx);
    }
}

/// After a tab switch: a list dropped while the tab was parked, or one whose tips moved, is
/// asked for again, and the page loads again in the background. An action's reply that came
/// while the tab was parked was only logged, so the panel no longer waits for it.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let loading = st.pull_request.as_ref().is_some_and(|v| v.files == Files::Loading);
    if let Some(v) = st.pull_request.as_mut() {
        v.busy = None;
    }
    retip(st, fx, loading);
    load(st, fx);
}

/// Reads the tips again from the snapshot; requests the list when they changed or `again`.
fn retip(st: &mut AppState, fx: &mut Vec<Effect>, again: bool) {
    let (Some(snap), Some(view)) = (&st.snap, &st.pull_request) else { return };
    let tips = Tips::of(snap, &view.remote, &view.pr);
    let moved = tips != view.tips;
    if moved {
        if shows_diff(st) {
            super::repo::close_diff(st);
        }
        if let Some(v) = st.pull_request.as_mut() {
            v.tips = tips;
        }
    }
    if moved || again {
        request(st, fx);
    }
}

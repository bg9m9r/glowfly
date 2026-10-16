//! The pull request panel: opening it for a listed pull request, loading the files it changes
//! from the local remote-tracking branches, following refreshes of the refs and of the
//! service's list, and closing it. Its files' diffs are the shared `st.diff`, opened through
//! `RepoMsg::OpenDiff`.

use crate::effect::Effect;
use crate::pull_request::{FilesJob, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::{Files, PrView, Tips};
use trellis_core::repo::DiffTarget;

pub(super) fn handle(st: &mut AppState, msg: PullRequestMsg, fx: &mut Vec<Effect>) {
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
            st.pull_request = Some(view);
            request(st, fx);
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
            fx.push(Effect::LoadPrFiles(FilesJob { workdir, id: view.pr.id, tips, generation }));
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

/// Closes the panel, and the diff when it is one of its files; a list still in flight must
/// not reopen it.
pub(super) fn close(st: &mut AppState) {
    let shown = shows_diff(st);
    if st.pull_request.take().is_some() {
        st.pull_request_gen += 1;
        if shown {
            super::repo::close_diff(st);
        }
    }
}

/// A diff is opening: the panel stays only when it is one of its files.
pub(super) fn keep_for(st: &mut AppState, target: &DiffTarget) {
    if !st.pull_request.as_ref().is_some_and(|v| v.shows(target)) {
        close(st);
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
pub(super) fn prs_changed(st: &mut AppState) {
    let Some(view) = st.pull_request.as_mut() else { return };
    match st.forge.as_ref().and_then(|f| f.prs.iter().find(|p| p.id == view.pr.id)) {
        Some(pr) => {
            view.pr = pr.clone();
            view.listed = true;
        }
        None => view.listed = false,
    }
}

/// After a tab switch: a list dropped while the tab was parked, or one whose tips moved, is
/// asked for again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let loading = st.pull_request.as_ref().is_some_and(|v| v.files == Files::Loading);
    retip(st, fx, loading);
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

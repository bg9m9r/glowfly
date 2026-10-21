//! A partial clone's missing file contents: a diff, File view or blame read that needs contents
//! the clone hasn't downloaded (`PartialMsg::ContentMissing`) waits in its pane while exactly
//! those objects download, then reads again. A trusted repository downloads in the background
//! (no prompts; the pane's Cancel stops it); an untrusted one waits for the pane's button, which
//! runs the download as a job through the trust gate. A newer read of another file, or a tab
//! switch, stops a background download.

use super::jobs::{JobSpec, Started, start_job};
use crate::effect::Effect;
use crate::effect::partial::{ContentRequest, PartialEffect, StopKey};
use crate::msg::ContentFailure;
use crate::state::AppState;
use crate::state::partial::{ContentPhase, ContentView, DOWNLOAD_LABEL, PendingContent, Wait};
use trellis_core::git_version::Feature;
use trellis_core::partial::{MissingContent, fetch_cmds};

/// A read for the view `retry` loads came back missing `missing`.
pub(super) fn missing(st: &mut AppState, missing: MissingContent, retry: Box<Effect>, fx: &mut Vec<Effect>) {
    let Some(view) = ContentView::of(&retry) else { return };
    if !st.content_view_current(&view) {
        return; // superseded while it read
    }
    let Some(workdir) = st.workdir() else { return };
    let MissingContent { oids, remote, path } = missing;
    // A newer read of the same view and file (a reload, a whitespace toggle) keeps the download.
    if let Some(p) = st.partial.content.as_mut()
        && p.view.same_kind(&view)
        && p.path == path
        && p.oids == oids
    {
        p.view = view;
        p.retry = Some(retry);
        return;
    }
    stop_background(st, fx);
    let wait = if oids.iter().all(|o| st.partial.fetched.contains(o)) {
        Some(Wait::StillMissing)
    } else if let Some(why) = st.git.blocked(Feature::FetchObjects) {
        Some(Wait::OldGit(why))
    } else if !st.runs_git_unasked() {
        Some(Wait::Trust)
    } else {
        None
    };
    let start = wait.is_none();
    // A background download is started right below.
    let phase = ContentPhase::Waiting(wait.unwrap_or(Wait::Cancelled));
    st.partial.content = Some(PendingContent { workdir, view, path, remote, oids, retry: Some(retry), phase, held: false });
    if start {
        fetch_in_background(st, fx);
    }
}

/// Starts the waiting view's download in the background.
fn fetch_in_background(st: &mut AppState, fx: &mut Vec<Effect>) {
    let id = st.partial.take_id();
    let Some(p) = st.partial.content.as_mut() else { return };
    p.phase = ContentPhase::Fetching { id, stopping: false };
    let request = ContentRequest { workdir: p.workdir.clone(), id, cmds: fetch_cmds(&p.remote, &p.oids) };
    fx.push(Effect::Partial(PartialEffect::FetchContent(request)));
}

/// Stops the background download, if one runs (another view's read replaces it).
fn stop_background(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(ContentPhase::Fetching { id, .. }) = st.partial.content.as_ref().map(|p| &p.phase) {
        fx.push(Effect::StopGit(StopKey::Content(*id)));
    }
}

/// The waiting pane's button: Download (an untrusted repository, as a job through the trust
/// gate) or Try again (in the background when trusted).
pub(super) fn download(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.partial.content.as_ref() else { return };
    if !matches!(st.content_phase(p), ContentPhase::Waiting(w) if !matches!(w, Wait::OldGit(_))) {
        return;
    }
    st.partial.fetched.retain(|o| !p.oids.contains(o));
    if st.runs_git_unasked() {
        return fetch_in_background(st, fx);
    }
    let cmds = fetch_cmds(&p.remote, &p.oids);
    let started = start_job(st, JobSpec::new(DOWNLOAD_LABEL.to_owned(), cmds), fx);
    if let Some(p) = st.partial.content.as_mut() {
        match started {
            Started::Now => p.phase = ContentPhase::FetchingAsJob,
            Started::Held => p.held = true,
            Started::No => {}
        }
    }
}

/// The downloading pane's Cancel.
pub(super) fn stop(st: &mut AppState, fx: &mut Vec<Effect>) {
    match st.partial.content.as_mut().map(|p| &mut p.phase) {
        Some(ContentPhase::Fetching { id, stopping }) if !*stopping => {
            *stopping = true;
            fx.push(Effect::StopGit(StopKey::Content(*id)));
        }
        Some(ContentPhase::FetchingAsJob) => super::job_stop::cancel(st, fx),
        _ => {}
    }
}

/// Background download `id` ended: the view reads again, or its pane says why it didn't.
pub(super) fn fetched(st: &mut AppState, id: u64, result: Result<(), ContentFailure>, fx: &mut Vec<Effect>) {
    let Some(p) = st.partial.content.as_mut().filter(|p| matches!(p.phase, ContentPhase::Fetching { id: mine, .. } if mine == id)) else {
        return;
    };
    match result {
        Ok(()) => downloaded(st, fx),
        Err(ContentFailure::Stopped) => p.phase = ContentPhase::Waiting(Wait::Cancelled),
        Err(ContentFailure::Failed(why)) => p.phase = ContentPhase::Waiting(Wait::Failed(why)),
    }
}

/// The contents came: the view reads again, and its result shows as usual (a read still
/// missing them says so instead of downloading again).
fn downloaded(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(mut p) = st.partial.content.take() else { return };
    st.partial.fetched.extend(p.oids.iter().copied());
    if let Some(retry) = p.retry.take().filter(|_| st.content_view_current(&p.view)) {
        fx.push(*retry);
    }
}

/// The active job ended (before `finish_job`): the download job started from a waiting pane
/// reads its view again, or its pane says why it didn't.
pub(super) fn job_done(st: &mut AppState, ok: bool, output: &str, fx: &mut Vec<Effect>) {
    if !is_download_job(st) {
        return;
    }
    if ok {
        downloaded(st, fx);
    } else if let Some(p) = st.partial.content.as_mut() {
        p.phase = ContentPhase::Waiting(Wait::Failed(trellis_core::logs::last_message(output)));
    }
}

/// The active job was cancelled: a download job's pane offers it again.
pub(super) fn job_stopped(st: &mut AppState) {
    if is_download_job(st)
        && let Some(p) = st.partial.content.as_mut()
    {
        p.phase = ContentPhase::Waiting(Wait::Cancelled);
    }
}

/// The active job is the waiting pane's download.
fn is_download_job(st: &AppState) -> bool {
    st.job.as_ref().is_some_and(|j| j.label == DOWNLOAD_LABEL)
        && st.partial.content.as_ref().is_some_and(|p| matches!(p.phase, ContentPhase::FetchingAsJob | ContentPhase::Waiting(Wait::Trust)))
}

/// The repository was trusted: a pane waiting for that downloads now, unless its job was the
/// one the trust prompt held (released right after this).
pub(super) fn trust_released(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.partial.content.as_mut().filter(|p| p.phase == ContentPhase::Waiting(Wait::Trust)) else { return };
    if std::mem::take(&mut p.held) {
        p.phase = ContentPhase::FetchingAsJob;
    } else {
        fetch_in_background(st, fx);
    }
}

/// The active tab is left: its size check and waiting contents are forgotten, and a background
/// download stops (the views read again, and ask again, when the tab returns).
pub(super) fn leave(st: &mut AppState, fx: &mut Vec<Effect>) {
    stop_background(st, fx);
    st.partial.content = None;
    st.partial.sizing = None;
    st.partial.fetched.clear();
}

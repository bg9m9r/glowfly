//! Where the pull request panel's files and commits come from: asking for them from the
//! remote-tracking tips and the listed head (the runner uses the head when it is already here),
//! asking again when that key changes, and, when no source tip is here at all (a fork, or a
//! merged branch that was deleted), fetching the service's head ref once in the background
//! without prompts, then comparing again. The notes' button fetches it with prompts.

use crate::effect::Effect;
use crate::pull_request::{FilesJob, HeadJob, PrEffect};
use crate::state::AppState;
use crate::state::pull_request::{Files, Tips};
use crate::state::pull_request_source::{HeadFetch, Shown, head_fetch_blocked};
use glowfly_core::git_version::Feature;
use glowfly_core::remote_ops::fetch_ref_cmd;
use glowfly_core::repo::PrCompare;

/// Asks for the files and commits of the open view's key; older results are ignored from now
/// on. Without the target's tip, or with neither a source branch nor a head, there is nothing
/// to compare.
pub(super) fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.pull_request_gen += 1;
    let generation = st.pull_request_gen;
    let workdir = st.workdir();
    let Some(view) = st.pull_request.as_mut() else { return };
    let key = view.files_key();
    view.asked = Some(key.clone());
    view.shown = None;
    let has_source = key.branch.is_some() || key.head.is_some();
    match (key.target, workdir) {
        (Some(target), Some(workdir)) if has_source => {
            view.files = Files::Loading;
            let job = FilesJob { workdir, id: view.pr.id, target, branch: key.branch, head: key.head, generation };
            fx.push(Effect::PullRequest(PrEffect::Files(job)));
        }
        _ => view.files = Files::Missing,
    }
}

/// Asks again when `again`, or when the key changed since the last ask (a diff of the old
/// lists closes then).
pub(super) fn rekey(st: &mut AppState, fx: &mut Vec<Effect>, again: bool) {
    let Some(view) = &st.pull_request else { return };
    let changed = view.asked.as_ref() != Some(&view.files_key());
    if changed && shows_diff(st) {
        super::super::repo::close_diff(st);
    }
    if changed || again {
        request(st, fx);
    }
}

/// Reads the tips again from the snapshot, then asks as `rekey` does.
pub(super) fn retip(st: &mut AppState, fx: &mut Vec<Effect>, again: bool) {
    let (Some(snap), Some(view)) = (&st.snap, &st.pull_request) else { return };
    let tips = Tips::of(snap, &view.remote, &view.pr);
    if let Some(v) = st.pull_request.as_mut() {
        v.tips = tips;
    }
    rekey(st, fx, again);
}

/// Whether the diff on screen is one of the open pull request's files or commits.
pub(super) fn shows_diff(st: &AppState) -> bool {
    match (&st.pull_request, &st.diff) {
        (Some(v), Some((t, _))) => v.shows(t),
        _ => false,
    }
}

/// The runner's comparison. With no source tip here, the head ref is fetched once.
pub(super) fn files_loaded(st: &mut AppState, id: u64, generation: u64, result: Result<PrCompare, String>, fx: &mut Vec<Effect>) {
    if generation != st.pull_request_gen {
        return; // superseded: closed, reopened, the key changed, or parked with its tab
    }
    let Some(view) = st.pull_request.as_mut().filter(|v| v.pr.id == id) else { return };
    match result {
        Ok(PrCompare::NoSource) => {
            view.files = Files::Missing;
            auto_head_fetch(st, fx);
        }
        Ok(PrCompare::Unrelated(source)) => {
            view.files = Files::Unrelated;
            view.shown = Some(Shown { source, commits: Default::default() });
        }
        Ok(PrCompare::Loaded { source, changes, commits }) => view.set_loaded(changes, source, commits),
        Err(e) => {
            view.files = Files::Failed(e.clone());
            st.log(false, e);
        }
    }
}

/// Fetches the head ref without prompts when the service keeps one, the head is known, the
/// repository is trusted, its remote can be fetched and isn't paused for sign-in, and this
/// panel copy hasn't tried yet.
fn auto_head_fetch(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(v), Some(snap), Some(workdir)) = (&st.pull_request, &st.snap, st.workdir()) else { return };
    let ready = v.head_ref.is_some()
        && v.files_key().head.is_some()
        && v.head_fetch == HeadFetch::Idle
        && st.runs_git_unasked()
        && crate::state::remotes::find(snap, &v.remote).is_some_and(|r| !r.fetch_url().is_empty())
        && !st.sign_in.is_paused(&workdir, &v.remote);
    if ready {
        start(st, false, fx);
    }
}

/// Starts the head ref fetch (`prompts`: the user asked for it).
fn start(st: &mut AppState, prompts: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    // Without --no-write-fetch-head (git 2.29) FETCH_HEAD is written.
    let write_fetch_head = st.git.blocked(Feature::FetchObjects).is_some();
    let Some(v) = st.pull_request.as_mut() else { return };
    let (Some(head_ref), Some(head)) = (v.head_ref.clone(), v.files_key().head) else { return };
    let cmd = fetch_ref_cmd(&v.remote, &head_ref, write_fetch_head);
    v.head_fetch = HeadFetch::Running;
    fx.push(Effect::PullRequest(PrEffect::FetchHead(HeadJob { workdir, id: v.pr.id, head, cmd, prompts })));
}

/// The notes' "Fetch #7's head" button: fetches with prompts, unless it is disabled.
pub(super) fn fetch_clicked(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(v), Some(snap)) = (&st.pull_request, &st.snap) else { return };
    if head_fetch_blocked(snap, v, st.runs_git_unasked()).is_some() {
        return;
    }
    start(st, true, fx);
}

/// The head ref fetch's result, for the panel copy that is still waiting for it with the same
/// listed head: compares again, or notes (and logs) the failure.
pub(super) fn head_fetched(st: &mut AppState, id: u64, head: String, result: Result<(), String>, fx: &mut Vec<Effect>) {
    let Some(v) = st.pull_request.as_mut() else { return };
    if v.pr.id != id || v.files_key().head.as_deref() != Some(head.as_str()) || v.head_fetch != HeadFetch::Running {
        return;
    }
    match result {
        Ok(()) => {
            v.head_fetch = HeadFetch::Done;
            request(st, fx);
        }
        Err(e) => {
            let text = format!("Couldn't fetch {}'s head from {}: {e}", v.label, v.remote);
            v.head_fetch = HeadFetch::Failed(e);
            st.log_quiet(false, text);
        }
    }
}

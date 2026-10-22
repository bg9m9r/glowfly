//! The pull request panel's calls to its hosting service: loading the page (detail and
//! conversation; the checks are in checks.rs), posting a comment, submitting a review, replying to and resolving code
//! comment threads (threads.rs), starting a code comment on diff lines (line_comment.rs) and merging, then deleting the source branch and fetching. The calls are unscoped effects whose replies name their repository,
//! service and pull request: a reply for another tab is only logged, a stale page is dropped,
//! and an action's result is logged even when the panel has closed, since it happened on the
//! service.

use super::super::ops::{JobSpec, busy, start_job};
use crate::effect::{Effect, ForgeError};
use crate::pull_request::{CallJob, PrCall, PrEffect, PrReply, PrRequest};
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction};
use crate::state::pull_request_actions::{comment_blocked, deletes_after_merge, merge_blocked, prefill, review_blocked};
use crate::state::pull_request_line_comment::adopt_new_threads;
use glowfly_core::forge::{self, MergeRequest, PrPage, ReviewEvent, Target};
use glowfly_core::gitcli::args;
use glowfly_core::remote_ops::fetch_cmd;
use glowfly_core::text::printable;
use std::path::Path;
use std::time::Instant;

/// A request to the active repository's service, or why there can be none.
pub(super) fn service_request(st: &AppState) -> Result<PrRequest, String> {
    let (Some(workdir), Some(fs)) = (st.workdir(), st.forge.as_ref()) else { return Err("No hosting service for this repository".into()) };
    let Some(target) = fs.host.clone() else { return Err("No hosting service for this repository".into()) };
    let account = target.account();
    if !forge::may_contact(&account, &st.settings.connections) {
        return Err(format!("not connected to {} at {}", account.provider.name(), account.display()));
    }
    let auth = fs.auth.get(&account.key()).cloned();
    let connected = forge::is_connected(&account, &st.settings.connections);
    Ok(PrRequest { workdir, target, auth, connected })
}

/// `service_request`, for a service with in-app reviews.
fn request(st: &AppState) -> Result<PrRequest, String> {
    let req = service_request(st)?;
    if !forge::reviews_in_app(req.target.provider) {
        return Err(format!("{} reviews aren't supported in Glowfly yet", req.target.provider.name()));
    }
    Ok(req)
}

/// Loads the open panel's page (again); the last page stays on screen meanwhile. Nothing is
/// asked of a service without in-app reviews, nor while the repository isn't trusted (signing
/// in may run its credential helper).
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(v) = st.pull_request.as_ref() else { return };
    if v.page == Page::Unsupported {
        return;
    }
    if !st.runs_git_unasked() {
        if let Some(v) = st.pull_request.as_mut()
            && v.loaded().is_none()
        {
            v.page = Page::Held;
        }
        return;
    }
    let req = request(st);
    st.pull_request_page_gen += 1;
    let generation = st.pull_request_page_gen;
    let Some(v) = st.pull_request.as_mut() else { return };
    match req {
        Err(e) => {
            if v.loaded().is_none() {
                v.page = Page::Failed(e);
            }
        }
        Ok(req) => {
            if v.loaded().is_some() {
                v.reloading = true;
            } else {
                v.page = Page::Loading;
            }
            fx.push(Effect::PullRequest(PrEffect::Call(Box::new(CallJob { req, id: v.pr.id, generation, call: PrCall::Load }))));
        }
    }
}

/// Sends `call` for `action` when the page is loaded and no other action is in flight.
pub(super) fn act(st: &mut AppState, action: PrAction, call: PrCall, fx: &mut Vec<Effect>) {
    let req = match request(st) {
        Ok(r) => r,
        Err(e) => {
            st.log(false, e);
            return;
        }
    };
    let generation = st.pull_request_page_gen;
    let Some(v) = st.pull_request.as_mut().filter(|v| v.loaded().is_some() && v.busy.is_none()) else { return };
    v.busy = Some(action);
    v.action_error = None;
    fx.push(Effect::PullRequest(PrEffect::Call(Box::new(CallJob { req, id: v.pr.id, generation, call }))));
}

pub(super) fn post_comment(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(v) = st.pull_request.as_ref().filter(|v| comment_blocked(v).is_none()) else { return };
    let call = PrCall::Comment(v.forms.comment.clone());
    act(st, PrAction::Comment, call, fx);
}

pub(super) fn submit_review(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(v) = st.pull_request.as_ref().filter(|v| review_blocked(v).is_none()) else { return };
    let Some(page) = v.loaded() else { return };
    let call = PrCall::Review { event: v.forms.review_event, body: v.forms.review_body.clone(), pending: page.detail.pending_review };
    act(st, PrAction::Review, call, fx);
}

pub(super) fn merge(st: &mut AppState, fx: &mut Vec<Effect>) {
    let default_branch = st.forge.as_ref().and_then(|f| f.default_branch.clone());
    let Some(v) = st.pull_request.as_ref().filter(|v| merge_blocked(v).is_none()) else { return };
    let (Some(page), Some(method)) = (v.loaded(), v.forms.merge_method) else { return };
    let req = MergeRequest {
        method,
        head_sha: page.detail.head_sha.clone(),
        title: v.forms.merge_title.trim().to_owned(),
        message: v.forms.merge_message.trim().to_owned(),
    };
    let delete = deletes_after_merge(v, default_branch.as_deref());
    let call = PrCall::Merge { req, remote: v.remote.clone(), source: v.pr.source.clone(), delete };
    act(st, PrAction::Merge, call, fx);
}

/// The log line for an action's reply; `into` names the target branch of a merge, if known.
pub(super) fn action_text(label: &str, into: &str, reply: &PrReply) -> (bool, String) {
    let fail = |what: &str, e: &ForgeError| (false, format!("{what} {label} failed: {}", e.message));
    match reply {
        PrReply::Loaded(_) | PrReply::Checks(_) => (true, String::new()),
        PrReply::Commented(Ok(_)) => (true, format!("Commented on {label}")),
        PrReply::Commented(Err(e)) => fail("Commenting on", e),
        PrReply::Reviewed(ReviewEvent::Approve, Ok(())) => (true, format!("Approved {label}")),
        PrReply::Reviewed(ReviewEvent::RequestChanges, Ok(())) => (true, format!("Requested changes on {label}")),
        PrReply::Reviewed(ReviewEvent::Comment, Ok(())) => (true, format!("Reviewed {label}")),
        PrReply::Reviewed(_, Err(e)) => fail("Reviewing", e),
        PrReply::Merged { result: Ok(_), method, .. } if into.is_empty() => (true, format!("Merged {label} ({})", method.api_name())),
        PrReply::Merged { result: Ok(_), method, .. } => (true, format!("Merged {label} into {into} ({})", method.api_name())),
        PrReply::Merged { result: Err(e), .. } => fail("Merging", e),
        PrReply::RepliedToThread { result: Ok(_), .. } => (true, format!("Replied on {label}")),
        PrReply::RepliedToThread { result: Err(e), .. } => fail("Replying on", e),
        PrReply::Resolved { result: Ok(true), .. } => (true, format!("Resolved a thread on {label}")),
        PrReply::Resolved { result: Ok(false), .. } => (true, format!("Reopened a thread on {label}")),
        PrReply::Resolved { resolve: true, result: Err(e), .. } => fail("Resolving a thread on", e),
        PrReply::Resolved { resolve: false, result: Err(e), .. } => fail("Reopening a thread on", e),
        PrReply::LineCommented { review: false, result: Ok(n) } => (true, format!("Commented on {} in {label}", printable(&n.thread.path))),
        PrReply::LineCommented { review: true, result: Ok(n) } => {
            (true, format!("Added a review comment on {} in {label}", printable(&n.thread.path)))
        }
        PrReply::LineCommented { result: Err(e), .. } => fail("Commenting on", e),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replied(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    id: u64,
    generation: u64,
    reply: PrReply,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let active = st.workdir().as_deref() == Some(workdir) && st.forge.as_ref().and_then(|f| f.host.as_ref()) == Some(target);
    let failed_auth = match &reply {
        PrReply::Loaded(Err(e)) | PrReply::Checks(Err(e)) | PrReply::Commented(Err(e)) | PrReply::Reviewed(_, Err(e)) => e.auth,
        PrReply::Merged { result: Err(e), .. }
        | PrReply::RepliedToThread { result: Err(e), .. }
        | PrReply::Resolved { result: Err(e), .. }
        | PrReply::LineCommented { result: Err(e), .. } => e.auth,
        _ => false,
    };
    if active && failed_auth {
        let key = target.account().key();
        if let Some(fs) = st.forge.as_mut() {
            fs.auth.remove(&key);
        }
    }
    let prefix = target.provider.pr_prefix();
    let label = format!("{prefix}{id}");
    let open = active && st.pull_request.as_ref().is_some_and(|v| v.pr.id == id);
    let reply = match reply {
        PrReply::Loaded(result) => {
            if open && generation == st.pull_request_page_gen {
                page_loaded(st, result);
                // A fork the page names isn't compared from a same-named branch.
                super::source::rekey(st, fx, false);
                // A new head or new threads' paths ask for new line maps.
                super::line_map::sync(st, fx, false);
            }
            return;
        }
        PrReply::Checks(result) => {
            if open && generation == st.pull_request_checks_gen {
                super::checks::loaded(st, result, now);
            }
            return;
        }
        r @ PrReply::LineCommented { .. } => {
            super::line_comment::replied(st, workdir, target, id, active, &label, r, now, fx);
            return;
        }
        other => other,
    };
    let into = match (&st.pull_request, &st.forge) {
        (Some(v), _) if open => v.pr.target.clone(),
        (_, Some(fs)) if active => fs.prs.iter().find(|p| p.id == id).map(|p| p.target.clone()).unwrap_or_default(),
        _ => String::new(),
    };
    let (ok, text) = action_text(&label, &into, &reply);
    if !active {
        st.log_quiet(ok, text);
        return;
    }
    st.log(ok, text.clone());
    if open && let Some(v) = st.pull_request.as_mut() {
        v.busy = None;
        if let Some(e) = reply_error(&reply) {
            let action = match &reply {
                PrReply::Commented(_) => PrAction::Comment,
                PrReply::Reviewed(..) => PrAction::Review,
                PrReply::RepliedToThread { thread, .. } => PrAction::Reply(thread.clone()),
                PrReply::Resolved { thread, .. } => PrAction::Resolve(thread.clone()),
                _ => PrAction::Merge,
            };
            v.action_error = Some((action, e.message.clone()));
        }
    }
    match reply {
        PrReply::Commented(Ok(entry)) => {
            if open && let Some(v) = st.pull_request.as_mut() {
                v.forms.comment.clear();
                if let Page::Loaded(p) = &mut v.page {
                    p.entries.push(entry);
                }
            }
            st.toast(true, "Comment posted", now);
        }
        PrReply::Reviewed(_, Ok(())) => {
            if open && let Some(v) = st.pull_request.as_mut() {
                v.forms.review_body.clear();
            }
            st.toast(true, text, now);
            if open {
                load(st, fx);
            }
            super::super::forge::refresh(st, true, false, now, fx);
        }
        PrReply::Merged { remote, source, delete, result: Ok(_), .. } => {
            st.toast(true, text, now);
            if open {
                load(st, fx);
            }
            super::super::forge::refresh(st, true, false, now, fx);
            after_merge(st, &remote, &source, delete, fx);
        }
        r @ (PrReply::RepliedToThread { result: Ok(_), .. } | PrReply::Resolved { result: Ok(_), .. }) => {
            super::threads::applied(st, open, r, text, now)
        }
        _ => {}
    }
}

fn reply_error(reply: &PrReply) -> Option<&ForgeError> {
    match reply {
        PrReply::Loaded(r) => r.as_ref().err(),
        PrReply::Checks(r) => r.as_ref().err(),
        PrReply::Commented(r) => r.as_ref().err(),
        PrReply::Reviewed(_, r) => r.as_ref().err(),
        PrReply::Merged { result, .. } => result.as_ref().err(),
        PrReply::RepliedToThread { result, .. } => result.as_ref().err(),
        PrReply::Resolved { result, .. } => result.as_ref().err(),
        PrReply::LineCommented { result, .. } => result.as_ref().err(),
    }
}

/// A page arrived for the open panel: show it and fit the forms to it, or say why it failed
/// (a failed reload keeps the last page and only logs).
fn page_loaded(st: &mut AppState, result: Result<PrPage, ForgeError>) {
    let Some(v) = st.pull_request.as_mut() else { return };
    v.reloading = false;
    let failed = match result {
        Ok(page) => {
            prefill(&mut v.forms, &page.detail);
            adopt_new_threads(v, &page);
            // Reply drafts stay; a focus on a thread that is gone doesn't.
            if v.threads.focus.as_ref().is_some_and(|f| !page.threads.iter().any(|t| &t.id == f)) {
                v.threads.focus = None;
            }
            v.page = Page::Loaded(Box::new(page));
            None
        }
        Err(e) if v.loaded().is_none() => {
            v.page = Page::Failed(e.message);
            None
        }
        Err(e) => Some(format!("Reloading {} {} failed: {}", v.noun, v.label, e.message)),
    };
    if let Some(text) = failed {
        st.log_quiet(false, text);
    }
}

/// After a merge: bring it into this repository with one git job (delete the source branch on
/// its remote when asked, then fetch with pruning), or say how when another job is running.
fn after_merge(st: &mut AppState, remote: &str, source: &str, delete: bool, fx: &mut Vec<Effect>) {
    if busy(st, true) {
        let mut hint = format!("Merged. Fetch {remote} to bring the merge into this repository.");
        if delete {
            hint.push_str(&format!(" Delete {source} from its remote branch menu."));
        }
        st.log(true, hint);
        return;
    }
    let mut cmds = Vec::new();
    let label = if delete {
        cmds.push(args(&["push", remote, "--delete", source]));
        format!("Delete {source} and fetch {remote}")
    } else {
        format!("Fetch {remote}")
    };
    cmds.push(fetch_cmd(remote));
    start_job(st, JobSpec::new(label, cmds), fx);
}

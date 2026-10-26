//! The issue panel: opening it for a listed issue (closing the pull request panel, a diff,
//! the file history and blame), loading its details from the tracker, following tab switches
//! and tracker changes, and closing it. The calls are unscoped effects whose replies name
//! their repository and tracker: a stale or misplaced reply is dropped, and a failed reload
//! keeps the details on screen. The Create issue dialog is in issue_create.rs, the comment box
//! in issue_comment.rs.

use crate::effect::{Effect, ForgeError, IssueCall, IssueEffect, IssueJob, IssueReply, IssueRequest};
use crate::msg::IssueMsg;
use crate::state::issue::{Details, IssueView, listed_or_bare};
use crate::state::{AppState, Modal};
use glowfly_core::forge::{self, IssueDetails, Target};
use std::path::Path;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: IssueMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        IssueMsg::Open(key) => open(st, key, fx),
        IssueMsg::Close => close(st),
        IssueMsg::Reload => load(st, fx),
        IssueMsg::CreateDialog => super::issue_create::open_dialog(st, fx),
        IssueMsg::SetProject(p) => super::issue_create::set_project(st, p, fx),
        IssueMsg::Create => super::issue_create::create(st, fx),
        IssueMsg::PostComment => super::issue_comment::post(st, fx),
        IssueMsg::Quote(i) => super::issue_comment::quote(st, i),
        IssueMsg::Replied { workdir, target, generation, reply } => match *reply {
            IssueReply::Loaded { key, result } => loaded(st, &workdir, &target, generation, &key, result),
            IssueReply::Options { project, result } => super::issue_create::options_replied(st, &workdir, &target, &project, result),
            IssueReply::Created(result) => super::issue_create::created(st, &workdir, &target, result, now, fx),
            IssueReply::Commented { key, result } => super::issue_comment::commented(st, &workdir, &target, &key, result, now, fx),
        },
    }
}

fn open(st: &mut AppState, key: String, fx: &mut Vec<Effect>) {
    let (Some(_), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.tracker.is_none() || st.session.issue.view.as_ref().is_some_and(|v| v.key == key && fs.tracker.as_ref() == Some(&v.target)) {
        return;
    }
    let Some(view) = IssueView::new(listed_or_bare(fs, &key), fs) else { return };
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    super::pull_request::close(st);
    st.session.issue.view = Some(view);
    load(st, fx);
}

/// A request to the active repository's tracker, or why there can be none.
pub(super) fn request(st: &AppState) -> Result<IssueRequest, String> {
    let none = || "No issue tracker for this repository".to_owned();
    let (Some(workdir), Some(fs)) = (st.workdir(), st.forge.as_ref()) else { return Err(none()) };
    let Some(target) = fs.tracker.clone() else { return Err(none()) };
    if !forge::issues_in_app(target.provider) {
        return Err(format!("{} issues aren't supported in Glowfly yet", target.provider.name()));
    }
    let account = target.account();
    if !forge::may_contact(&account, &st.settings.connections) {
        return Err(format!("not connected to {} at {}", account.provider.name(), account.display()));
    }
    let auth = fs.auth.get(&account.key()).cloned();
    let connected = forge::is_connected(&account, &st.settings.connections);
    Ok(IssueRequest { workdir, target, auth, connected })
}

/// Loads the open panel's details (again); the last ones stay on screen meanwhile. Nothing is
/// asked of a tracker without issue details in Glowfly, nor while the repository isn't trusted
/// (signing in may run its credential helper).
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(v) = st.session.issue.view.as_ref() else { return };
    if v.details == Details::Unsupported {
        return;
    }
    if !st.runs_git_unasked() {
        if let Some(v) = st.session.issue.view.as_mut()
            && v.loaded().is_none()
        {
            v.details = Details::Held;
        }
        return;
    }
    let req = request(st);
    st.session.issue.generation += 1;
    let generation = st.session.issue.generation;
    let Some(v) = st.session.issue.view.as_mut() else { return };
    match req {
        Err(e) => {
            if v.loaded().is_none() {
                v.details = Details::Failed(e);
            }
        }
        Ok(req) => {
            if v.loaded().is_some() {
                v.reloading = true;
            } else {
                v.details = Details::Loading;
            }
            let call = IssueCall::Load(v.key.clone());
            fx.push(Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation, call }))));
        }
    }
}

/// Details arrived: shown when they are for the open panel's issue and tracker and the latest
/// request; a failed reload keeps the details on screen and is only logged.
fn loaded(st: &mut AppState, workdir: &Path, target: &Target, generation: u64, key: &str, result: Result<IssueDetails, ForgeError>) {
    let tracker = st.forge.as_ref().and_then(|f| f.tracker.as_ref());
    let current = st.workdir().as_deref() == Some(workdir) && generation == st.session.issue.generation && tracker == Some(target);
    if !current || !st.session.issue.view.as_ref().is_some_and(|v| v.key == key && &v.target == target) {
        return;
    }
    if let Err(e) = &result
        && e.auth
        && let Some(fs) = st.forge.as_mut()
    {
        fs.auth.remove(&target.account().key());
    }
    let Some(v) = st.session.issue.view.as_mut() else { return };
    v.reloading = false;
    let failed = match result {
        Ok(d) => {
            v.issue = d.issue.clone();
            let issue = d.issue.clone();
            v.details = Details::Loaded(Box::new(d));
            if let Some(fs) = st.forge.as_mut() {
                fs.cache.insert(issue.key.clone(), issue);
            }
            return;
        }
        Err(e) => {
            let text = format!("Loading {} {} failed: {}", v.noun, v.mention, e.message);
            if v.loaded().is_none() {
                v.details = Details::Failed(e.message);
            }
            text
        }
    };
    st.log_quiet(false, failed);
}

/// Closes the panel; details still in flight must not reopen it.
pub(super) fn close(st: &mut AppState) {
    if st.session.issue.view.take().is_some() {
        st.session.issue.generation += 1;
    }
}

/// After a tab switch: the details load again in the background (the last ones stay meanwhile).
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    load(st, fx);
    super::issue_create::reload_pending(st, fx);
}

/// The issue tracker was chosen again (Repo-Specific Preferences): the panel closes, and so
/// does a Create issue dialog for another tracker.
pub(super) fn tracker_changed(st: &mut AppState) {
    close(st);
    tracker_detected(st);
}

/// The tracker was worked out again (after a refresh, a new connection or a profile switch):
/// a panel or a Create issue dialog for another tracker closes, since its calls would now go
/// to the new one.
pub(super) fn tracker_detected(st: &mut AppState) {
    let tracker = st.forge.as_ref().and_then(|f| f.tracker.clone());
    if st.session.issue.view.as_ref().is_some_and(|v| Some(&v.target) != tracker.as_ref()) {
        close(st);
    }
    if let Some(Modal::CreateIssue(form)) = &st.modal
        && Some(&form.target) != tracker.as_ref()
    {
        st.modal = None;
    }
}

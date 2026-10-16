//! The Create issue dialog: opening it for the active repository's tracker, loading the
//! labels, people, types and epics it offers (per Jira project), creating the issue, and the
//! reply: logged and toasted with a View action, cached, the ISSUES list refreshed, and the New
//! branch dialog opened when asked. A failure keeps the dialog open with the server's words;
//! a reply after the dialog closed (or for another tab) is only logged.

use crate::effect::{Effect, ForgeError, IssueCall, IssueEffect, IssueJob};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState, create_blocked};
use crate::state::{AppState, Modal, ToastAction};
use std::path::Path;
use std::time::Instant;
use trellis_core::forge::{self, CreateOptions, Issue, Target};

fn form_mut(st: &mut AppState) -> Option<&mut IssueForm> {
    match st.modal.as_mut() {
        Some(Modal::CreateIssue(f)) => Some(f),
        _ => None,
    }
}

/// Opens the dialog, unless creating is blocked (the controls say why) or another dialog is up.
pub(super) fn open_dialog(st: &mut AppState, fx: &mut Vec<Effect>) {
    if create_blocked(st).is_some() || st.modal.is_some() {
        return;
    }
    let Some(target) = st.forge.as_ref().and_then(|f| f.tracker.clone()) else { return };
    st.modal = Some(Modal::CreateIssue(Box::new(IssueForm::new(target))));
    ask_options(st, fx);
}

/// Asks for the choices of the dialog's project; on Jira only once a project is chosen.
fn ask_options(st: &mut AppState, fx: &mut Vec<Effect>) {
    let req = super::issue::request(st);
    let generation = st.issue_gen;
    let Some(form) = form_mut(st) else { return };
    let project = form.project.clone();
    if form.provider().is_jira() && project.is_empty() {
        form.options = OptionsState::NotAsked;
        return;
    }
    match req {
        Err(e) => form.options = OptionsState::Failed(e),
        Ok(req) => {
            form.options = OptionsState::Loading;
            form.options_for = project.clone();
            fx.push(Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation, call: IssueCall::Options(project) }))));
        }
    }
}

/// The project field was committed: another project's choices load (and a failed load is
/// asked for again).
pub(super) fn set_project(st: &mut AppState, project: String, fx: &mut Vec<Effect>) {
    let Some(form) = form_mut(st) else { return };
    let project = project.trim().to_owned();
    if project != form.project {
        form.set_project(&project);
    } else if !matches!(form.options, OptionsState::Failed(_)) {
        return;
    }
    ask_options(st, fx);
}

/// Sends the dialog's issue, unless it has problems or is already being created.
pub(super) fn create(st: &mut AppState, fx: &mut Vec<Effect>) {
    let req = super::issue::request(st);
    let generation = st.issue_gen;
    let Some(form) = form_mut(st) else { return };
    if form.busy || form.submit_blocked().is_some() {
        return;
    }
    match req {
        Err(e) => form.error = Some(e),
        Ok(req) => {
            form.busy = true;
            form.error = None;
            let call = IssueCall::Create(Box::new(form.to_new()));
            fx.push(Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation, call }))));
        }
    }
}

/// The choices arrived: applied to the open dialog of the same tracker and project.
pub(super) fn options_replied(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    project: &str,
    result: Result<CreateOptions, ForgeError>,
) {
    if st.workdir().as_deref() != Some(workdir) {
        return;
    }
    let Some(form) = form_mut(st).filter(|f| &f.target == target && f.options_for == project && f.project == project) else { return };
    match result {
        Ok(opts) => form.options_loaded(opts),
        Err(e) => form.options = OptionsState::Failed(e.message),
    }
}

/// "#12" or "PROJ-12" on `target`.
fn label_of(target: &Target, key: &str) -> String {
    forge::mention(target.provider.ref_style(), key)
}

pub(super) fn created(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    result: Result<Issue, ForgeError>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if st.workdir().as_deref() != Some(workdir) {
        let text = match &result {
            Ok(i) => {
                let folder = workdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                format!("Created issue {} in {folder}", label_of(target, &i.key))
            }
            Err(e) => format!("Creating the issue failed: {}", e.message),
        };
        st.log_quiet(result.is_ok(), text);
        return;
    }
    let waiting = form_mut(st).is_some_and(|f| &f.target == target && f.busy);
    match result {
        Ok(issue) => {
            let text = if target.provider.is_jira() {
                format!("Created {}: {}", issue.key, issue.title)
            } else {
                format!("Created issue #{}: {}", issue.key, issue.title)
            };
            st.log(true, text.clone());
            let open = Msg::Issue(IssueMsg::Open(issue.key.clone()));
            st.toast_action(true, text, ToastAction { label: "View".into(), msg: Box::new(open), workdir: Some(workdir.to_owned()) }, now);
            let branch = waiting && form_mut(st).is_some_and(|f| f.then_branch);
            let name = forge::branch_name_for(&issue);
            if let Some(fs) = st.forge.as_mut().filter(|f| f.tracker.as_ref() == Some(target)) {
                fs.cache.insert(issue.key.clone(), issue);
            }
            super::forge::refresh(st, true, false, now, fx);
            if branch {
                st.modal = Some(Modal::NewBranch { start: None, name, checkout: true });
            } else if waiting {
                st.modal = None;
            }
        }
        Err(e) => {
            st.log(false, format!("Creating the issue failed: {}", e.message));
            if e.auth
                && let Some(fs) = st.forge.as_mut()
            {
                fs.auth.remove(&target.account().key());
            }
            match form_mut(st).filter(|_| waiting) {
                Some(form) => {
                    form.busy = false;
                    form.error = Some(e.message);
                }
                None => st.toast(false, format!("Couldn't create the issue: {}", e.message), now),
            }
        }
    }
}

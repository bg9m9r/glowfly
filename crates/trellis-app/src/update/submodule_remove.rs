//! Removing a submodule: reading it again (`SubmoduleEffect::CheckRemove`), the confirmation
//! naming what goes and what would be lost, the removal as a job (`SubmoduleEffect::Remove`)
//! and its result, with its own messages for each partial failure.

use super::ops::{self, Started};
use super::submodules::find;
use crate::effect::{Effect, SubmoduleEffect};
use crate::state::submodules::remove::{
    RemoveForm, git_part_headline, job_label, module_left_text, remove_blocked, removed_headline, rm_failed_headline,
};
use crate::state::{AppState, Modal};
use crate::submodule::RemoveRequest;
use std::time::Instant;
use trellis_core::submodule_ops::{RemovalFacts, RemoveOutcome, RemoveReport};

/// The row menu's Remove submodule…: reads the submodule again before asking.
pub(super) fn dialog(st: &mut AppState, path: String, fx: &mut Vec<Effect>) {
    let Some(sm) = find(st, &path) else { return };
    if let Some(why) = st.snap.as_ref().and_then(|s| remove_blocked(&sm, s)) {
        return st.log(false, format!("Can't remove {path}: {why}"));
    }
    let Some(workdir) = st.workdir() else { return };
    fx.push(Effect::Submodule(SubmoduleEffect::CheckRemove { workdir, path }));
}

/// What the runner read: opens the confirmation, or says why not.
pub(super) fn checked(st: &mut AppState, path: String, result: Result<Option<Box<RemovalFacts>>, String>) {
    let facts = match result {
        Ok(Some(f)) => f,
        Ok(None) => return st.log(false, format!("Submodule {path} is no longer in this repository.")),
        Err(e) => return st.log(false, format!("Could not read submodule {path}: {e}")),
    };
    // Keep the row current with what was just read.
    if let Some(row) = st.snap.as_mut().and_then(|s| s.submodules.iter_mut().find(|m| m.path == path)) {
        *row = facts.info.clone();
    }
    if let Some(why) = st.snap.as_ref().and_then(|s| remove_blocked(&facts.info, s)) {
        return st.log(false, format!("Can't remove {path}: {why}"));
    }
    st.modal = Some(Modal::RemoveSubmodule(RemoveForm::from_facts(&facts)));
}

/// The confirmation's button: checks the submodule again and starts the removal job.
pub(super) fn confirm(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::RemoveSubmodule(form)) = st.modal.as_ref() else { return };
    if form.needs_ack() && !form.acked {
        tracing::debug!("removing submodule {} waits for the acknowledgement", form.path);
        return;
    }
    let path = form.path.clone();
    let Some(sm) = find(st, &path) else {
        st.modal = None;
        return;
    };
    if let Some(why) = st.snap.as_ref().and_then(|s| remove_blocked(&sm, s)) {
        return st.log(false, format!("Can't remove {path}: {why}"));
    }
    let label = job_label(&path);
    let effect = move |repo| Effect::Submodule(SubmoduleEffect::Remove(RemoveRequest { repo, path, name: sm.name }));
    // Removing changes the index, so the undo history is cleared, as for other such jobs.
    if ops::begin_job(st, &label, false, effect, fx) == Started::Now {
        st.modal = None;
    }
}

/// The removal finished: one toast and log entry for the outcome, plus a separate failure for a
/// stored module that stayed behind.
pub(super) fn removed(st: &mut AppState, request: RemoveRequest, report: RemoveReport, now: Instant, fx: &mut Vec<Effect>) {
    if st.job.is_none() {
        return;
    }
    let path = request.path.as_str();
    let RemoveReport { outcome, output } = report;
    let (ok, headline, extra) = match &outcome {
        RemoveOutcome::Removed => (true, Some(removed_headline(path)), None),
        RemoveOutcome::DeinitFailed => (false, None, None),
        RemoveOutcome::RmFailed => (false, Some(rm_failed_headline(path, ops::last_line(&output))), None),
        RemoveOutcome::ModuleLeft { dir, error } => (true, Some(git_part_headline(path)), Some(module_left_text(path, dir, error))),
    };
    ops::finish_job(st, ok, output, headline, None, now, fx);
    if let Some(text) = extra {
        st.toast(false, text.clone(), now);
        st.log(false, text);
    }
}

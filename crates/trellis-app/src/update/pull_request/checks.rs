//! The pull request panel's Checks section: loading every check on the head commit when the
//! panel opens, on Try again, when the service's list says they changed, and once a minute while
//! one is still running; and opening or closing the section. The calls are unscoped effects like
//! the page's (calls.rs); a reply for another tab, a closed panel or an older request is dropped.

use super::calls::service_request;
use crate::effect::{Effect, ForgeError};
use crate::pull_request::{CallJob, PrCall, PrEffect};
use crate::state::AppState;
use crate::state::pull_request_checks::ChecksPane;
use std::time::Instant;
use trellis_core::forge::CheckList;

/// Loads the open panel's checks (again); the last list stays on screen meanwhile. Nothing is
/// asked of a service without them, nor while the repository isn't trusted (signing in may run
/// its credential helper).
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(v) = st.pull_request.as_ref() else { return };
    if v.checks.pane == ChecksPane::Unsupported {
        return;
    }
    if !st.runs_git_unasked() {
        if let Some(v) = st.pull_request.as_mut()
            && v.checks.loaded().is_none()
        {
            v.checks.pane = ChecksPane::Held;
        }
        return;
    }
    let req = match service_request(st) {
        Ok(req) => req,
        Err(e) => {
            if let Some(v) = st.pull_request.as_mut()
                && v.checks.loaded().is_none()
            {
                v.checks.pane = ChecksPane::Failed(e);
            }
            return;
        }
    };
    st.pull_request_checks_gen += 1;
    let generation = st.pull_request_checks_gen;
    let Some(v) = st.pull_request.as_mut() else { return };
    v.checks.in_flight = true;
    if v.checks.loaded().is_some() {
        v.checks.reloading = true;
    } else {
        v.checks.pane = ChecksPane::Loading;
    }
    let call = PrCall::Checks { head: v.pr.head.clone() };
    fx.push(Effect::PullRequest(PrEffect::Call(Box::new(CallJob { req, id: v.pr.id, generation, call }))));
}

/// The checks arrived for the open panel: show them, or say why they failed (a failed reload
/// keeps the last list and only logs).
pub(super) fn loaded(st: &mut AppState, result: Result<CheckList, ForgeError>, now: Instant) {
    let Some(v) = st.pull_request.as_mut() else { return };
    let c = &mut v.checks;
    c.in_flight = false;
    c.reloading = false;
    c.fetched = Some(now);
    let failed = match result {
        Ok(list) => {
            c.pane = ChecksPane::Loaded(list);
            None
        }
        Err(e) if c.loaded().is_none() => {
            c.pane = ChecksPane::Failed(e.message);
            None
        }
        Err(e) => Some(format!("Reloading checks of {} failed: {}", v.label, e.message)),
    };
    if let Some(text) = failed {
        st.log_quiet(false, text);
    }
}

/// Reads a list with a check still running again, once a minute.
pub(super) fn tick(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.pull_request.as_ref().is_some_and(|v| v.checks.due(now)) {
        load(st, fx);
    }
}

/// The section's header: open or close it.
pub(super) fn toggle(st: &mut AppState) {
    if let Some(v) = st.pull_request.as_mut() {
        v.checks.open = !v.checks.open;
    }
}

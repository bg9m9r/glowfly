//! Creating a stack's pull requests: the stack is pushed to the code host's remote first when a
//! layer isn't there at its tip, then the ordinary create dialog opens for each layer in turn,
//! bottom first, targeting the branch below (`ForgeState::stack_prs`). A layer whose pull
//! request the service created opens the next dialog; Skip moves on; Cancel, Esc, the close
//! button, Discard, a failed create, a missing target, another dialog or a parked tab stop the
//! run, each saying so in the log.

use super::rebase::refuse;
use crate::effect::Effect;
use crate::state::stack_prs::{PrLayer, StackPrRun, StackPrs, pr_layers, prs_blocker};
use crate::state::stack_push::{layers, needs_push, remote_tip, targets};
use crate::state::{AppState, Modal, push_pr};
use std::time::Instant;

/// Starts the run for the stack `branch` is in, unless it is refused; pushes first when needed.
pub(super) fn start(st: &mut AppState, branch: String, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return };
    let Some(fs) = st.forge.as_ref().filter(|f| f.host.is_some()) else {
        let remote = snap.locals.iter().find(|b| b.name == branch).and_then(|b| push_pr::pr_remote(b, snap));
        refuse(st, push_pr::no_host_reason(remote.as_deref()), now);
        return;
    };
    let names = layers(&snap.stacks, &branch);
    if names.is_empty() {
        st.log(false, t.trf("stack.not_in_stack", &[("branch", &branch)]));
        return;
    }
    if let Some(why) = prs_blocker(t, snap, fs, &branch) {
        refuse(st, why, now);
        return;
    }
    if needs_push(snap, &targets(snap, &names, Some(&fs.remote))) {
        let remote = fs.remote.clone();
        super::stack_push::push(st, branch.clone(), Some(remote), Some(branch), now, fx);
    } else {
        begin(st, &branch, fx);
    }
}

/// The dialogs of the stack `branch` is in start with its bottom layer.
fn begin(st: &mut AppState, branch: &str, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    let left: std::collections::VecDeque<PrLayer> = pr_layers(snap, fs, branch).into();
    let run = StackPrRun { total: left.len(), left, ..Default::default() };
    if let Some(fs) = st.forge.as_mut() {
        fs.stack_prs = Some(StackPrs::Running(run));
    }
    advance(st, fx);
}

/// Opens the next layer's dialog, skipping layers that have an open pull request by now; the
/// run ends when none is left.
fn advance(st: &mut AppState, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    loop {
        let Some(fs) = st.forge.as_mut() else { return };
        let Some(StackPrs::Running(mut run)) = fs.stack_prs.take() else { return };
        run.current = None;
        run.sent = false;
        let Some(layer) = run.left.pop_front() else {
            let names = |v: &[String]| if v.is_empty() { t.tr("stack.prs.none").to_owned() } else { v.join(", ") };
            let text = t.trf("stack.prs.finished", &[("created", &names(&run.created)), ("skipped", &names(&run.skipped))]);
            st.log(true, text);
            return;
        };
        run.index = run.total - run.left.len();
        if let Some(pr) = fs.pr_for_branch(&layer.source) {
            let (noun, label) = (fs.pr_word(t), fs.pr_label(pr.id));
            run.skipped.push(layer.branch.clone());
            fs.stack_prs = Some(StackPrs::Running(run));
            st.log(true, t.trf("stack.prs.skipped_open", &[("branch", &layer.branch), ("noun", &noun), ("pr", &label)]));
            continue;
        }
        let remote = fs.remote.clone();
        let missing = layer.target.as_ref().filter(|target| st.snap.as_ref().is_none_or(|s| remote_tip(s, &remote, target).is_none()));
        let why = match (missing, &st.modal) {
            (Some(target), _) => {
                Some(t.trf("stack.prs.target_missing", &[("target", target), ("remote", &remote), ("branch", &layer.branch)]))
            }
            (None, Some(_)) => Some(t.tr("stack.prs.other_dialog").to_owned()),
            (None, None) => None,
        };
        run.current = Some(layer.clone());
        fs.stack_prs = Some(StackPrs::Running(run));
        match why {
            Some(why) => stop_with(st, Some(why)),
            None => super::push_pr::open_dialog(st, layer.branch, layer.target, None, fx),
        }
        return;
    }
}

/// A snapshot landed: a run waiting for its push starts.
pub(super) fn loaded(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    if !matches!(fs.stack_prs, Some(StackPrs::AfterPush(_))) {
        return;
    }
    if let Some(StackPrs::AfterPush(branch)) = fs.stack_prs.take() {
        begin(st, &branch, fx);
    }
}

/// The run's current layer, when its dialog is the one for `source`.
fn current_is(st: &AppState, source: &str) -> bool {
    st.forge.as_ref().and_then(|f| f.stack_step(source)).is_some()
}

fn running(st: &mut AppState) -> Option<&mut StackPrRun> {
    match st.forge.as_mut()?.stack_prs.as_mut()? {
        StackPrs::Running(run) => Some(run),
        StackPrs::AfterPush(_) => None,
    }
}

/// The create dialog for `source` was sent to the service.
pub(super) fn sending(st: &mut AppState, source: &str) {
    if current_is(st, source)
        && let Some(run) = running(st)
    {
        run.sent = true;
    }
}

/// The create dialog for `source` was refused before it reached the service.
pub(super) fn refused(st: &mut AppState, source: &str) {
    if current_is(st, source) {
        stop(st);
    }
}

/// The service created pull requests from `sources`: the current layer's moves the run on.
pub(super) fn created(st: &mut AppState, sources: &[String], fx: &mut Vec<Effect>) {
    let Some(run) = running(st) else { return };
    let Some(current) = run.current.as_ref().filter(|c| sources.contains(&c.source)) else { return };
    let branch = current.branch.clone();
    run.created.push(branch);
    run.current = None;
    advance(st, fx);
}

/// Creating a pull request failed: the run stops if it was the current layer's.
pub(super) fn create_failed(st: &mut AppState) {
    if running(st).is_some_and(|r| r.sent) {
        stop(st);
    }
}

/// The stack step's Skip: the next layer's dialog.
pub(super) fn skip(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(run) = running(st) else { return };
    let Some(current) = run.current.take() else { return };
    run.skipped.push(current.branch);
    advance(st, fx);
}

/// The stack step's Cancel: the run stops.
pub(super) fn stop(st: &mut AppState) {
    stop_with(st, None);
}

/// Ends the run, logging `reason` first, then the layers it didn't create.
fn stop_with(st: &mut AppState, reason: Option<String>) {
    let t = st.settings.lang();
    let Some(prs) = st.forge.as_mut().and_then(|f| f.stack_prs.take()) else { return };
    if let Some(why) = reason {
        st.log(false, why);
    }
    let left = match prs {
        StackPrs::Running(run) => run.not_created(),
        StackPrs::AfterPush(_) => Vec::new(),
    };
    if !left.is_empty() {
        st.log(false, t.trf("stack.prs.stopped", &[("branches", &left.join(", "))]));
    }
}

/// The open dialog is being dismissed (Esc, its close button, or Discard on the question): the
/// run stops when it is the current layer's dialog and its Create wasn't sent.
pub(super) fn dismissed(st: &mut AppState) {
    let Some(Modal::CreatePr { source, .. }) = &st.modal else { return };
    let source = source.clone();
    if current_is(st, &source) && running(st).is_some_and(|r| !r.sent) {
        stop(st);
    }
}

/// The active tab is being parked: its run is dropped.
pub(super) fn parked(st: &mut AppState) {
    if st.forge.as_ref().is_some_and(|f| f.stack_prs.is_some()) {
        let why = st.settings.lang().tr("stack.prs.tab_changed").to_owned();
        stop_with(st, Some(why));
    }
}

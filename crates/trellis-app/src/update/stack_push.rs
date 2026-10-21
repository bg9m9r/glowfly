//! Pushing a stack as the single active job: refusing what can't run, pushing every layer to
//! its target in one job (`Effect::Stack`), and ending it with one log line per layer and a
//! headline. A push started for the stack's pull requests hands over to `stack_prs` once it
//! lands. Pushes move no local branch, so the undo history is kept.

use super::rebase::refuse;
use crate::effect::{Effect, StackEffect, StackPushJob};
use crate::msg::StackMsg;
use crate::state::AppState;
use crate::state::stack_prs::StackPrs;
use crate::state::stack_push::{layers, push_blocker, targets};
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::paths::same_spelling;
use trellis_core::stack::{LayerOutcome, LayerPushed, PushPlan, StackPush};

pub const JOB_LABEL: &str = "Push stack";

pub(super) fn handle(st: &mut AppState, msg: StackMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        StackMsg::Push { branch } => push(st, branch, None, None, now, fx),
        StackMsg::CreatePrs { branch } => super::stack_prs::start(st, branch, now, fx),
        StackMsg::Pushed { result, job } => pushed(st, result, *job, now, fx),
        StackMsg::SkipPr => super::stack_prs::skip(st, fx),
        StackMsg::StopPrs => super::stack_prs::stop(st),
    }
}

/// Starts the push of the stack `branch` is in, every layer to `remote` (None: each to its own
/// push target). From the pull request flow (`then_prs`), which checked its own blockers and
/// pushes only when something needs it, Push stack's own blockers aren't asked again: a
/// diverged layer is reported per layer.
pub(super) fn push(
    st: &mut AppState,
    branch: String,
    remote: Option<String>,
    then_prs: Option<String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return };
    let names = layers(&snap.stacks, &branch);
    if names.is_empty() {
        st.log(false, t.trf("stack.not_in_stack", &[("branch", &branch)]));
        return;
    }
    if then_prs.is_none()
        && let Some(why) = push_blocker(t, snap, &branch)
    {
        refuse(st, why, now);
        return;
    }
    let job = StackPushJob { targets: targets(snap, &names, remote.as_deref()), then_prs };
    super::ops::begin_job(st, JOB_LABEL, true, move |cwd| Effect::Stack(StackEffect::Push { cwd, job }), fx);
}

/// The push job ended: the job's log and toast, then one line per layer; a push for the
/// stack's pull requests that landed in the active tab starts them once the refresh lands.
fn pushed(st: &mut AppState, result: Result<StackPush, String>, job: StackPushJob, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    // The user may have switched tabs while the push ran: its lines go to the pushed repo's tab.
    let repo = st.job.as_ref().and_then(|j| j.workdir.clone());
    let here = match (&repo, st.workdir()) {
        (Some(a), Some(b)) => same_spelling(a, &b),
        _ => false,
    };
    let ok = match result {
        Err(e) => {
            super::ops::finish_job(st, false, e, None, None, now, fx);
            false
        }
        Ok(run) => {
            let headline = if run.ok() {
                t.tr("stack.push.done").to_owned()
            } else {
                t.trf("stack.push.partial", &[("pushed", &run.landed()), ("total", &run.layers.len())])
            };
            let ok = run.ok();
            super::ops::finish_job(st, ok, run.output.clone(), Some(headline), None, now, fx);
            for l in &run.layers {
                let landed = matches!(l.outcome, LayerOutcome::Pushed { .. } | LayerOutcome::UpToDate);
                st.log_in(repo.as_deref(), landed, layer_line(t, l));
            }
            ok
        }
    };
    let Some(branch) = job.then_prs else { return };
    match st.forge.as_mut() {
        Some(fs) if ok && here => fs.stack_prs = Some(StackPrs::AfterPush(branch)),
        _ => {
            let remote = job.targets.first().map(|t| t.remote.clone()).unwrap_or_default();
            st.log_in(repo.as_deref(), false, t.trf("stack.prs.not_pushed", &[("remote", &remote)]));
        }
    }
}

/// What became of one layer, for the log.
fn layer_line(t: Lang, l: &LayerPushed) -> String {
    let p = &l.target;
    let (branch, remote, dest) = (&p.branch, &p.remote, &p.dest);
    let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", branch), ("remote", remote), ("dest", dest)];
    match &l.outcome {
        LayerOutcome::Pushed { plan: PushPlan::New, upstream: true } => t.trf("stack.push.layer.new", args),
        LayerOutcome::Pushed { plan: PushPlan::New, .. } => t.trf("stack.push.layer.new_untracked", args),
        LayerOutcome::Pushed { plan: PushPlan::Rewritten, .. } => t.trf("stack.push.layer.forced", args),
        LayerOutcome::Pushed { .. } => t.trf("stack.push.layer.pushed", args),
        LayerOutcome::UpToDate => t.trf("stack.push.layer.up_to_date", args),
        LayerOutcome::NotPushed(PushPlan::Behind) => t.trf("stack.push.layer.behind", args),
        LayerOutcome::NotPushed(PushPlan::Diverged) => t.trf("stack.push.layer.diverged", args),
        LayerOutcome::NotPushed(PushPlan::Gone) => t.trf("stack.push.layer.gone", args),
        LayerOutcome::NotPushed(_) | LayerOutcome::Failed => t.trf("stack.push.layer.failed", args),
        LayerOutcome::Stale => t.trf("stack.push.layer.stale", args),
        LayerOutcome::Rejected(reason) => t.trf("stack.push.layer.rejected", &[("branch", branch), ("remote", remote), ("reason", reason)]),
    }
}

//! Previewing an operation before it runs: a Preview… item opens the dialog and asks the runner
//! for the result (`Effect::OpPreview`), Preview again asks once more against the branch as it
//! is now, and Apply closes the dialog and sends exactly what the direct menu item sends. No
//! trust gate: the preview is asked for by the user and only reads, like a conflict check from
//! the branch menu.

use crate::effect::{Effect, OpPreviewJob};
use crate::msg::OpPreviewMsg;
use crate::state::op_preview::{PreviewDialog, apply_blocker, apply_msg, resolve};
use crate::state::{AppState, Modal};
use glowfly_core::git_version::Feature;
use glowfly_core::op_preview::{PreviewOp, Request};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: OpPreviewMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        OpPreviewMsg::Open(op) => open(st, op, fx),
        OpPreviewMsg::Loaded { job, result } => {
            let Some(Modal::OpPreview(d)) = &mut st.modal else { return };
            if d.job != job || d.result.is_some() {
                return;
            }
            if let Err(e) = &result {
                let line = st.settings.lang().trf("op_preview.failed", &[("error", &glowfly_core::error::plain_message(e))]);
                d.result = Some(result.map(|b| *b));
                st.log(false, line);
            } else {
                d.result = Some(result.map(|b| *b));
            }
        }
        OpPreviewMsg::Again => again(st, fx),
        OpPreviewMsg::Apply(mode) => {
            let (Some(Modal::OpPreview(d)), Some(snap)) = (&st.modal, &st.snap) else { return };
            let t = st.settings.lang();
            if let Some(why) = apply_blocker(t, st.job.is_some(), snap, d) {
                st.log(false, why);
                return;
            }
            let Some(msg) = apply_msg(t, snap, &d.job.request.op, mode) else { return };
            st.modal = None;
            fx.extend(super::update(st, msg, now));
        }
    }
}

/// Opens the dialog for `op` against HEAD now and asks for its result.
fn open(st: &mut AppState, op: PreviewOp, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let Some(head) = snap.head_oid else {
        st.log(false, "Commit something first to preview it.");
        return;
    };
    let job = OpPreviewJob { workdir: snap.workdir.clone(), request: Request { op, head } };
    st.palette = None;
    start(st, job, fx);
}

/// Shows `job` in the dialog: at once with git's version as its error for a merge on a git
/// without merge-tree, otherwise loading while the runner works it out.
fn start(st: &mut AppState, job: OpPreviewJob, fx: &mut Vec<Effect>) {
    let too_old = matches!(job.request.op, PreviewOp::Merge { .. }).then(|| st.git.blocked(Feature::MergeTree)).flatten();
    let result = too_old.map(Err);
    if result.is_none() {
        fx.push(Effect::OpPreview(job.clone()));
    }
    st.modal = Some(Modal::OpPreview(Box::new(PreviewDialog { job, result })));
}

/// Preview again: the same operation with the commits the snapshot shows now.
fn again(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(Modal::OpPreview(d)), Some(snap)) = (&st.modal, &st.snap) else { return };
    let Some(head) = snap.head_oid else {
        st.log(false, "Commit something first to preview it.");
        return;
    };
    let op = match &d.job.request.op {
        PreviewOp::Merge { theirs: name, .. } | PreviewOp::Rebase { onto: name, .. } => {
            let Some(oid) = resolve(snap, name) else {
                let line = format!("There is no branch {name}.");
                st.log(false, line);
                return;
            };
            match &d.job.request.op {
                PreviewOp::Merge { .. } => PreviewOp::Merge { theirs: name.clone(), oid },
                _ => PreviewOp::Rebase { onto: name.clone(), oid },
            }
        }
        other => other.clone(),
    };
    let job = OpPreviewJob { workdir: snap.workdir.clone(), request: Request { op, head } };
    start(st, job, fx);
}

//! Applying a patch file: the open dialog, reading the chosen file (no git) for the Apply
//! patch dialog, then one `RunGit` job: `git apply` with the chosen options, or `git am` for
//! the series as commits. The headlines of those jobs and of the Applying patches banner's
//! buttons.

use super::super::ops::{self, JobSpec};
use crate::effect::Effect;
use crate::patch::{ApplyHow, ApplyRequest, PatchEffect, PatchMsg};
use crate::state::am_banner::is_am_label;
use crate::state::patch::apply::{self as form, ApplyForm};
use crate::state::patch::{AM_LABEL, APPLY_LABEL, PendingApply, display_name};
use crate::state::{AppState, Modal};
use git2::RepositoryState;
use std::path::Path;
use std::time::Instant;
use trellis_core::patch_ops;

pub(super) fn handle(st: &mut AppState, m: PatchMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        PatchMsg::PickToApply => {
            let Some(workdir) = st.workdir() else { return };
            if ops::busy(st, false) {
                return;
            }
            fx.push(Effect::Patch(PatchEffect::PickOpen { workdir }));
        }
        PatchMsg::ApplyFrom { path: None, .. } => {}
        PatchMsg::ApplyFrom { workdir, path: Some(path) } => fx.push(Effect::Patch(PatchEffect::Inspect { workdir, path })),
        PatchMsg::Inspected { workdir, path, result } => {
            if !still_active(st, &workdir) {
                return;
            }
            match result {
                Ok(summary) => st.modal = Some(Modal::ApplyPatch(Box::new(ApplyForm::new(workdir, path, summary)))),
                Err(e) => {
                    let name = display_name(&path);
                    let text = if e.starts_with(&name) { e } else { format!("Can't read {name}: {e}") };
                    st.toast(false, &text, now);
                    st.log(false, text);
                }
            }
        }
        PatchMsg::Apply(req) => apply(st, req, fx),
        _ => {}
    }
}

/// Whether `workdir`, the repository a dialog was opened for, is still the active tab; logs
/// why not.
fn still_active(st: &mut AppState, workdir: &Path) -> bool {
    let active = st.workdir().is_some_and(|w| trellis_core::paths::same_spelling(&w, workdir));
    if !active {
        st.log(false, format!("The patch was not applied: {} is no longer the active tab.", display_name(workdir)));
    }
    active
}

/// Starts the job `req` asks for, unless its repository is no longer the active tab, it can't
/// run now (said in the log; the dialog stays open) or a job is running.
fn apply(st: &mut AppState, req: ApplyRequest, fx: &mut Vec<Effect>) {
    if !still_active(st, &req.workdir) {
        return;
    }
    // Command lines are Strings; a name that isn't valid Unicode can't be passed on.
    let Some(text) = req.path.to_str() else {
        st.log(false, format!("Trellis can't pass this file name to git: {}.", req.path.display()));
        return;
    };
    if let Some(why) = st.snap.as_ref().and_then(|s| form::request_blocked(&req, s)) {
        st.log(false, why);
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    let (label, cmds) = match req.how {
        ApplyHow::Changes(o) => (APPLY_LABEL, patch_ops::apply_cmds(text, &o)),
        ApplyHow::Commits { three_way } => (AM_LABEL, vec![patch_ops::am_cmd(text, three_way)]),
    };
    // Also for a job waiting for the trust prompt; dropping it clears this.
    if ops::start_job(st, JobSpec::new(label.into(), cmds), fx) != ops::Started::No {
        st.patch_apply = Some(PendingApply { label: label.into(), name: display_name(&req.path), how: req.how, commits: req.commits });
        if matches!(st.modal, Some(Modal::ApplyPatch(_))) {
            st.modal = None;
        }
    }
}

/// The headline of the job that just ended, when it is the pending apply job (which is always
/// cleared) or one of the Applying patches banner's buttons. `st.snap` is still the snapshot
/// from before the job.
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.patch_apply.take();
    let label = st.job.as_ref().map(|j| j.label.clone())?;
    if let Some(p) = pending.filter(|p| p.label == label) {
        let name = p.name.as_str();
        return Some(match (p.how, ok) {
            (ApplyHow::Changes(o), true) => form::changes_done(name, &o),
            (ApplyHow::Changes(o), false) => form::changes_failed(name, &o, output),
            (ApplyHow::Commits { .. }, true) => form::am_done(name, p.commits),
            (ApplyHow::Commits { .. }, false) => form::am_failed(Some(name), output),
        });
    }
    let applying = st.snap.as_ref().is_some_and(|s| s.state == RepositoryState::ApplyMailbox);
    (applying && is_am_label(&label)).then(|| if ok { form::am_resume_done(&label) } else { form::am_failed(None, output) })
}

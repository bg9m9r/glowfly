//! Keeping a branch's remote copy in step. Rename with the remote: renamed here, the new name
//! pushed with -u (the upstream moves to it) and only after that push the old name deleted on the
//! remote, leased on the tip last fetched; one job, so a failed step stops the rest, and its
//! headline says what was done and what is left (`headline`). Try again is offered only when the
//! rename itself failed (update/job_retry.rs), so neither push runs twice. Delete remote branch:
//! its dialog names the local branches that track the branch and offers to unset their
//! upstreams, which happens after the delete, so they stay when it fails. Neither is recorded
//! for undo, as the plain rename and remote delete aren't.

use super::jobs::{JobSpec, Started, start_job};
use crate::effect::Effect;
use crate::state::branch_sync::{DeleteRemoteForm, RenameJob, remote_rename};
use crate::state::{AppState, Modal, remote_branch};
use glowfly_core::branch_ops;
use glowfly_core::gitflow::failed_step;
use glowfly_core::repo::BranchInfo;

/// The job label of a rename that moves the branch on its remote too.
const RENAME_LABEL: &str = "Rename local and remote branch";

/// Renames local branch `b` to `new` (already checked as a local name) here and on its upstream's
/// remote, unless `remote_rename` refuses it (logged; the dialog stays open).
pub(super) fn rename_with_remote(st: &mut AppState, b: &BranchInfo, new: &str, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = st.snap.as_ref() else { return };
    let plan = match remote_rename(t, snap, b, new) {
        Some(Ok(plan)) => plan,
        Some(Err(why)) => {
            st.log(false, why);
            return;
        }
        None => {
            st.log(false, t.trf("branch_sync.rename.no_upstream", &[("branch", &b.name)]));
            return;
        }
    };
    if matches!(st.modal, Some(Modal::RenameBranch(_))) {
        st.modal = None;
    }
    let cmds = branch_ops::rename_with_remote_cmds(&b.name, new, &plan.remote, &plan.old, plan.tip);
    // Also for a job held for the trust prompt; a later job of another label ignores it.
    if start_job(st, JobSpec::new(RENAME_LABEL.into(), cmds), fx) != Started::No {
        st.rename_job = Some(RenameJob { label: RENAME_LABEL.into(), old: b.name.clone(), new: new.to_owned(), plan });
    }
}

/// The headline of the job that just ended, when it is the pending rename with the remote. A
/// failed rename (nothing else ran) keeps git's own explanation and the pending job, so Try
/// again, which runs the same commands under the same label, still gets this headline.
pub(super) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let job = st.job.as_ref()?;
    let pending = st.rename_job.as_ref().filter(|p| p.label == job.label)?;
    let step = failed_step(output, 3);
    if !ok && step == 0 {
        return None;
    }
    let why = if ok { String::new() } else { reason(st, job, output) };
    let line = pending.headline(st.settings.lang(), ok, step, &why);
    st.rename_job = None;
    line
}

/// Why the job failed, in plain words when Glowfly knows git's cause, else git's error line.
fn reason(st: &AppState, job: &crate::state::RunningJob, output: &str) -> String {
    let explained = super::job_errors::explain(st, job, None, None, output).map(|e| e.text);
    let error = || output.lines().rfind(|l| l.starts_with("error: ") || l.starts_with("fatal: ")).map(str::to_owned);
    let why = explained.or_else(error).unwrap_or_else(|| super::ops::last_line(output).to_owned());
    why.trim_end().trim_end_matches('.').to_owned()
}

/// Opens the Delete remote branch dialog for `remote`/`name`, unless the branch is gone.
pub(super) fn delete_remote_dialog(st: &mut AppState, remote: &str, name: &str) {
    let Some(snap) = st.snap.as_ref() else { return };
    if remote_branch::exists(snap, remote, name).is_none() {
        gone(st, remote, name);
        return;
    }
    st.modal = Some(Modal::DeleteRemoteBranch(DeleteRemoteForm::new(snap, remote, name)));
}

/// Deletes `name` on `remote`, then unsets the upstream of each of `unset` that still tracks it.
pub(super) fn delete_remote(st: &mut AppState, remote: &str, name: &str, unset: &[String], fx: &mut Vec<Effect>) {
    if matches!(st.modal, Some(Modal::DeleteRemoteBranch(_))) {
        st.modal = None;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    if remote_branch::exists(snap, remote, name).is_none() {
        gone(st, remote, name);
        return;
    }
    let still = |b: &&String| snap.locals.iter().any(|l| l.name == **b && remote_branch::tracks(l, remote, name));
    let unset: Vec<String> = unset.iter().filter(still).cloned().collect();
    start_job(st, JobSpec::new("Delete remote branch".into(), branch_ops::delete_remote_cmds(remote, name, &unset)), fx);
}

fn gone(st: &mut AppState, remote: &str, name: &str) {
    let t = st.settings.lang();
    st.log(false, t.trf("branch_sync.delete.gone", &[("upstream", &format!("{remote}/{name}"))]));
}

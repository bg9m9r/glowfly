//! After a merge from the pull request panel: one git job brings it into this repository (the
//! source branch deleted on its remote when asked, then a fetch with pruning) and deletes the
//! local branch when asked (its rules and commands are in state/pull_request_cleanup.rs), or the
//! log says how when another job is running. A local branch that can't go any more (the
//! working tree changed since the form was filled in) is kept with a line saying why, and the
//! rest still runs. The job's headline says why the branch was kept when its local part stops.

use super::super::ops::{JobSpec, Started, busy, start_job};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::pull_request_cleanup::{CleanupJob, LocalDelete, Merging, local_delete, local_delete_cmds, restore_upstream};
use glowfly_core::forge::MergeMethod;
use glowfly_core::gitcli::args;
use glowfly_core::remote_ops::fetch_cmd;
use glowfly_core::undo::UndoKind;

/// Starts the clean-up after a merge of `source` (`delete`: delete it on `remote` first). `local`
/// is the target branch and the method when the local branch is to go too.
pub(super) fn after_merge(
    st: &mut AppState,
    remote: &str,
    source: &str,
    delete: bool,
    local: Option<(&str, MergeMethod)>,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    // Without the target's name there is nothing to check the branch against.
    let plan = local.filter(|(target, _)| !target.is_empty()).and_then(|(target, method)| {
        let snap = st.snap.as_ref()?;
        let default_branch = st.session.forge.as_ref().and_then(|f| f.default_branch.as_deref());
        // A fork's source, by the service's list or its page, is never the local branch here. With
        // the panel closed: the merge only asked for the local delete of a same-repository pull
        // request, and the target check still guards it.
        let view = st.session.pull_request.view.as_ref();
        let same_repo = view.is_none_or(|v| v.pr.head_repo.is_same() && v.loaded().is_none_or(|p| p.detail.same_repo));
        let m = Merging { remote, source, target, method: Some(method), same_repo, default_branch };
        let (branch, plan) = local_delete(t, snap, &m)?;
        Some((target, branch, plan))
    });
    if busy(st, true) {
        let mut hint = format!("Merged. Fetch {remote} to bring the merge into this repository.");
        if delete {
            hint.push_str(&format!(" Delete {source} from its remote branch menu."));
        }
        if let Some((_, _, Ok(d))) = &plan {
            hint.push(' ');
            hint.push_str(&t.trf("pr.cleanup.busy", &[("branch", &d.branch)]));
        }
        st.log(true, hint);
        return;
    }
    let mut cmds = Vec::new();
    if delete {
        cmds.push(args(&["push", remote, "--delete", source]));
    }
    cmds.push(fetch_cmd(remote));
    let local = match plan {
        Some((target, _, Ok(d))) => Some((target, d)),
        Some((_, branch, Err(why))) => {
            st.log(false, t.trf("pr.cleanup.kept", &[("branch", &branch), ("why", &why)]));
            None
        }
        None => None,
    };
    let Some((target, d)) = local else {
        let label = if delete { format!("Delete {source} and fetch {remote}") } else { format!("Fetch {remote}") };
        start_job(st, JobSpec::new(label, cmds), fx);
        return;
    };
    start_local(st, remote, source, target, delete, cmds, d, fx);
}

/// Starts the job with the local branch's deletion after `remote_cmds`, recorded for Undo, and
/// keeps its steps for the headline.
#[allow(clippy::too_many_arguments)]
fn start_local(
    st: &mut AppState,
    remote: &str,
    source: &str,
    target: &str,
    delete: bool,
    remote_cmds: Vec<Vec<String>>,
    d: LocalDelete,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    let branch = d.branch.as_str();
    let label = if delete {
        t.trf("pr.cleanup.label.delete", &[("source", &source), ("remote", &remote), ("branch", &branch)])
    } else {
        t.trf("pr.cleanup.label.fetch", &[("remote", &remote), ("branch", &branch)])
    };
    let mut steps = vec![None; remote_cmds.len()];
    let mut cmds = remote_cmds;
    for (step, cmd) in local_delete_cmds(&d, remote, target) {
        steps.push(Some(step));
        cmds.push(cmd);
    }
    let restore = restore_upstream(&d);
    let (branch, remote, target) = (d.branch.clone(), remote.to_owned(), target.to_owned());
    let job = CleanupJob { label: label.clone(), branch, remote, target, steps, restore: restore.clone() };
    // A failed job leaves the branch: its upstream goes back to the source.
    let spec =
        JobSpec { record: Some(UndoKind::DeleteBranch(d.branch.clone())), on_fail_check: Some(restore), ..JobSpec::new(label, cmds) };
    if start_job(st, spec, fx) != Started::No {
        st.pr_cleanup = Some(job);
    }
}

/// The headline of the job that just ended, when it is the pending clean-up and its local part
/// stopped. Always clears the pending job.
pub(in crate::update) fn headline(st: &mut AppState, ok: bool, output: &str) -> Option<String> {
    let pending = st.pr_cleanup.take()?;
    if ok || st.job.as_ref().is_none_or(|j| j.label != pending.label) {
        return None;
    }
    pending.stopped(st.settings.lang(), output)
}

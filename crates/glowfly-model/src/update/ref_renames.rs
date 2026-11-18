//! Hidden, shown-only and pinned-colour choices follow the renames Glowfly makes: a branch
//! renamed (also its remote-tracking copy when the same job renamed it on the remote), a remote
//! renamed, and their undo and redo. A removed remote's entries go with it, and the job's
//! headline says so. Applied from the job's result before the reload (`ops::finish_job`), so the
//! load already carries the moved names and prunes nothing. Pins have no remote in their key:
//! only a branch rename moves one. Renames made outside Glowfly are left to the load, which
//! drops the entries (`graph_filter::loaded`).

use crate::effect::{Effect, UndoResult};
use crate::state::{AppState, RunningJob};
use glowfly_core::repo::Snapshot;
use glowfly_core::undo::Action;

/// What a finished job changed in the names of refs.
enum Change<'a> {
    /// Branch `from` is now `to`; `remote` is the remote it was renamed on too.
    Branch {
        from: &'a str,
        to: &'a str,
        remote: Option<&'a str>,
    },
    Remote {
        from: &'a str,
        to: &'a str,
    },
    RemoteGone(&'a str),
}

fn change<'a>(undo: &'a Option<UndoResult>, ok: bool) -> Option<Change<'a>> {
    match undo.as_ref()? {
        // The change happened even when a later step failed; only the remote copy needs `ok`.
        UndoResult::Recorded(Some(Action::RenameBranch { old, new, remote })) => {
            Some(Change::Branch { from: old, to: new, remote: remote.as_deref().filter(|_| ok) })
        }
        UndoResult::Recorded(Some(Action::RenameRemote { old, new, .. })) => Some(Change::Remote { from: old, to: new }),
        UndoResult::Recorded(Some(Action::RemoveRemote { name, .. })) => Some(Change::RemoteGone(name)),
        // An undo or redo of a branch rename runs `branch -m` alone, never on the remote.
        UndoResult::Done { redo, before: Action::RenameBranch { old, new, .. }, .. } => {
            let (from, to) = if *redo { (old, new) } else { (new, old) };
            Some(Change::Branch { from, to, remote: None })
        }
        UndoResult::Done { redo, before: Action::RenameRemote { old, new, .. }, .. } => {
            let (from, to) = if *redo { (old, new) } else { (new, old) };
            Some(Change::Remote { from, to })
        }
        // Undoing the removal writes the config back; the remote-tracking refs return with a fetch.
        UndoResult::Done { redo: true, before: Action::RemoveRemote { name, .. }, .. } => Some(Change::RemoteGone(name)),
        _ => None,
    }
}

/// Moves the saved choices of the repository `job` ran in for the change its result reports,
/// and saves the settings once. Does nothing for other jobs.
pub(super) fn job_finished(st: &mut AppState, job: &RunningJob, ok: bool, here: bool, undo: &Option<UndoResult>, fx: &mut Vec<Effect>) {
    let (Some(change), Some(workdir)) = (change(undo, ok), job.workdir.as_deref()) else { return };
    let real = st.repo_key(workdir);
    let before = st.settings.graph_filter_for(&real);
    let mut f = before.clone();
    let mut pins = false;
    match change {
        Change::Branch { from, to, remote } => {
            f.rename(&format!("refs/heads/{from}"), &format!("refs/heads/{to}"));
            if let Some(r) = remote
                && let Some(up) = upstream_of(st, job, here, from, r)
            {
                f.rename(&format!("refs/remotes/{r}/{up}"), &format!("refs/remotes/{r}/{to}"));
            }
            pins = st.settings.rename_branch_pin(&real, from, to);
        }
        Change::Remote { from, to } => {
            f.rename_remote(from, to);
        }
        Change::RemoteGone(name) => {
            f.remove_remote(name);
        }
    }
    let moved = f != before;
    if moved {
        st.settings.set_graph_filter(&real, f);
    }
    // No colour sync here: the reload applies the pins to the new snapshot, and syncing now
    // would drop the colour from the old name, still shown, for a frame.
    if moved || pins {
        fx.extend(super::prefs::save_settings(st));
    }
}

/// The name `branch` tracks on `remote`, from the snapshot the job started from (the reload has
/// not happened yet): the active one, or the one of the tab the job ran in.
fn upstream_of(st: &AppState, job: &RunningJob, here: bool, branch: &str, remote: &str) -> Option<String> {
    let snap: &Snapshot = if here {
        st.snap.as_ref()?
    } else {
        let workdir = job.workdir.as_deref()?;
        st.tabs.iter().find(|t| t.is(workdir))?.parked.as_ref()?.snap.as_ref()?
    };
    let t = snap.locals.iter().find(|b| b.name == branch)?.tracking.as_ref()?;
    (t.is_remote() && t.remote == remote).then(|| t.merge.clone())
}

/// The headline of a finished removal of a remote that took hidden or shown-only choices with
/// it. Only looks: `job_finished` drops them.
pub(super) fn headline(st: &AppState, undo: &Option<UndoResult>) -> Option<String> {
    let name = match undo.as_ref()? {
        UndoResult::Recorded(Some(Action::RemoveRemote { name, .. }))
        | UndoResult::Done { redo: true, before: Action::RemoveRemote { name, .. }, .. } => name,
        _ => return None,
    };
    let workdir = st.job.as_ref()?.workdir.as_deref()?;
    let (h, s) = st.settings.graph_filter_for(&st.repo_key(workdir)).remove_remote(name);
    (h + s > 0).then(|| st.settings.lang().trn("refs.filter.remote_removed", h + s, &[("remote", name)]))
}

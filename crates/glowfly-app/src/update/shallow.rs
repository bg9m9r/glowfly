//! Shallow clones: fetching more history as a job, and what a reload after a deepen invalidates.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::state::{AppState, Selection};
use glowfly_core::repo::Snapshot;
use glowfly_core::shallow_ops::{Deepen, deepen_cmd};

/// Fetches `how` much more of the open shallow clone's history from its remote, as the active
/// job (finish_job reloads the repository afterwards). The full history is sized first
/// (update/partial_size.rs), and a large one asks before it downloads.
pub(super) fn deepen(st: &mut AppState, how: Deepen, fx: &mut Vec<Effect>) {
    let Some(remote) = checked(st, how) else { return };
    if how == Deepen::Full {
        super::partial_size::start(st, remote, fx);
    } else {
        start_job(st, JobSpec::new(how.job_label(), vec![deepen_cmd(&remote, how)]), fx);
    }
}

/// `deepen` without the size check: the confirmation's button, and a size that isn't large.
pub(super) fn deepen_now(st: &mut AppState, how: Deepen, fx: &mut Vec<Effect>) {
    let Some(remote) = checked(st, how) else { return };
    start_job(st, JobSpec::new(how.job_label(), vec![deepen_cmd(&remote, how)]), fx);
}

/// The remote to fetch `how` more history from, or None (said in the log) when there is
/// nothing to fetch or nowhere to fetch it from.
fn checked(st: &mut AppState, how: Deepen) -> Option<String> {
    let snap = st.snap.as_ref()?;
    // A palette left open, or a header click, from before a reload that completed the history.
    if !snap.is_shallow() {
        st.log(true, "This repository already has its full history.");
        return None;
    }
    if how == Deepen::By(0) {
        st.log(false, "Fetch at least one more commit.");
        return None;
    }
    let Some(remote) = crate::state::shallow::history_remote(snap) else {
        st.log(false, "This repository has no remote to fetch its history from.");
        return None;
    };
    Some(remote)
}

/// Called from a refresh before `new` replaces `st.snap`: commits that were shallow boundaries
/// had their line counts worked out as root commits, so those are dropped, and a selected one's
/// detail is loaded again (its parents are there now).
pub(super) fn refreshed(st: &mut AppState, new: &Snapshot, fx: &mut Vec<Effect>) {
    let Some(old) = &st.snap else { return };
    let gone: Vec<git2::Oid> = old.shallow_roots.iter().filter(|o| !new.shallow_roots.contains(o)).copied().collect();
    for oid in gone {
        st.session.reads.stats.remove(&oid);
        st.session.reads.stats_pending.remove(&oid);
        st.session.reads.line_counts.forget_commit(oid);
        if st.selection == Selection::Commit(oid)
            && new.row_of.contains_key(&oid)
            && let Some(workdir) = st.workdir()
        {
            fx.push(Effect::LoadDetail { workdir, oid });
            super::line_counts::want_commit(st, oid, fx);
        }
    }
}

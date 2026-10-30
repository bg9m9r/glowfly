//! A job that stopped on conflicts says how many files are conflicted: first from git's CONFLICT
//! lines, then, once the status read after the job lands, from its unmerged entries
//! (state/conflict_files.rs). The toast, while it is up, and the run's summary take the new
//! count; a status read that started before the job ended, fails, never arrives or lists no
//! conflicts leaves the first count as it is.

use crate::state::conflict_files::{Recount, unmerged};
use crate::state::logs::EntryId;
use crate::state::{AppState, RunningJob};
use glowfly_core::paths::same_spelling;

/// The conflicts toast just shown (the newest) and run `entry` of `job` wait for the status.
pub(super) fn expect(st: &mut AppState, job: &RunningJob, entry: EntryId) {
    let (Some(workdir), Some(toast)) = (job.workdir.clone(), st.toasts.last().map(|t| t.id)) else { return };
    st.logs.recount = Some(Recount { toast, entry, workdir, generation: st.load_gen });
}

/// The active tab's status was read (of load generation `st.load_gen`): a waiting toast of
/// this repository takes its count. Any read of a later load ends the wait.
pub(super) fn status_read(st: &mut AppState) {
    if st.logs.recount.as_ref().is_none_or(|r| st.load_gen <= r.generation) {
        return;
    }
    let Some(r) = st.logs.recount.take() else { return };
    let n = st.snap.as_ref().filter(|s| same_spelling(&s.workdir, &r.workdir)).map_or(0, unmerged);
    if n == 0 {
        return;
    }
    let text = crate::state::error_words::conflicts(st.settings.lang(), n);
    if let Some(t) = st.toasts.iter_mut().find(|t| t.id == r.toast) {
        t.text.clone_from(&text);
    }
    if let Some(run) = st.log.iter_mut().find(|e| e.id == r.entry).and_then(|e| e.run.as_mut()) {
        run.summary = text;
    }
}

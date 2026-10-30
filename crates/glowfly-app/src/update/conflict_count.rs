//! A job that stopped on conflicts says how many files are conflicted: first from git's CONFLICT
//! lines, then from the first status read after the job (state/conflict_files.rs `Recount`).
//! That read's unmerged entries give the count, each path once, and the files, in git's order
//! (`glowfly_core::conflict_lines::ordered`), so the toast while it is up, the run's summary
//! and View conflicts take them. The first read after the job ends the wait whatever it found:
//! one that failed, of another repository or with no conflicts leaves git's count as it was, and
//! nothing rewrites the toast after it. The next job that ends loudly ends the wait too.

use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict_files::{Recount, unmerged};
use crate::state::logs::EntryId;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::conflict_lines::ordered;
use glowfly_core::paths::same_spelling;

/// The conflicts toast just shown (the newest) and run `entry` of `job` wait for the status;
/// `named` and `unsure` are the paths git's lines gave.
pub(super) fn expect(st: &mut AppState, job: &RunningJob, entry: EntryId, named: Vec<String>, unsure: Vec<Vec<String>>) {
    let (Some(workdir), Some(toast)) = (job.workdir.clone(), st.toasts.last().map(|t| t.id)) else { return };
    st.logs.recount = Some(Recount { toast, entry, workdir, generation: st.load_gen, named, unsure });
}

/// The active tab's status was read for load generation `st.load_gen` (`ok`: the read worked
/// and the snapshot has it). The first read of a load started after the job ends the wait; a
/// working one of the job's repository that lists conflicts gives the count and the files.
pub(super) fn status_read(st: &mut AppState, ok: bool) {
    if st.logs.recount.as_ref().is_none_or(|r| st.load_gen <= r.generation) {
        return;
    }
    let Some(r) = st.logs.recount.take() else { return };
    let here = st.snap.as_ref().filter(|s| ok && same_spelling(&s.workdir, &r.workdir));
    let Some((n, paths)) = here.map(unmerged).filter(|(n, _)| *n > 0) else { return };
    let t = st.settings.lang();
    let text = crate::state::error_words::conflicts(t, n);
    let toast = st.toasts.iter_mut().find(|x| x.id == r.toast);
    let up = toast.is_some();
    if let Some(x) = toast {
        x.text.clone_from(&text);
    }
    if let Some(run) = st.log.iter_mut().find(|e| e.id == r.entry).and_then(|e| e.run.as_mut()) {
        run.summary = text;
    }
    let Some(first) = ordered(&r.named, &r.unsure, &paths).into_iter().next() else { return };
    let view = ToastAction {
        label: t.tr("details.banner.view_conflicts.button").to_owned(),
        msg: Box::new(Msg::Conflict(ConflictMsg::OpenConflict(first))),
        workdir: Some(r.workdir),
    };
    match st.logs.fixes.iter_mut().find(|(id, _)| *id == r.entry) {
        Some((_, fix)) if matches!(*fix.msg, Msg::Conflict(ConflictMsg::OpenConflict(_))) => *fix = view,
        Some(_) => {}
        // git named none of the files: the toast, while up, gets View conflicts before Details.
        None if up => {
            let proxy = st.logs.keep_fix(r.entry, view);
            if let Some(x) = st.toasts.iter_mut().find(|x| x.id == r.toast) {
                x.details = x.details.take().or_else(|| x.action.take());
                x.action = Some(proxy);
            }
        }
        None => {}
    }
}

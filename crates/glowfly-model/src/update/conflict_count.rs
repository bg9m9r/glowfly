//! A job that stopped on conflicts says how many files are conflicted: first from git's CONFLICT
//! lines, then from the first complete status read after the job (state/conflict_files.rs
//! `Recount`). That read's unmerged entries give the count, each path once, and the files, in
//! git's order (`glowfly_core::conflict_lines::ordered`), so the toast while it is up, the run's
//! summary and View conflicts take them. Names that aren't UTF-8 are counted and listed, but
//! View conflicts never opens one: it opens the first file git can be given, or the WIP panel's
//! Conflicted Files list when there is none.
//!
//! A partial read (the watcher's path read, an index read) keeps the conflicted rows from before
//! the job, so it keeps git's count, says once that git's list may be incomplete, and keeps
//! waiting. A failed read, a read of another repository, a complete one with no conflicts or the
//! next job that ends loudly ends the wait, leaving the text as it was.

use crate::effect::StatusRead;
use crate::msg::{ConflictMsg, Msg, RepoMsg};
use crate::state::conflict_files::{Recount, unmerged};
use crate::state::logs::EntryId;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::conflict_lines::ordered;
use glowfly_core::error::ReadFailed;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::Hazard;

/// What a status read after a job can tell the recount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Coverage {
    /// The read failed.
    Failed,
    /// Only some paths were read; conflicted rows were kept from before (the watcher's path
    /// read, an index read).
    Partial,
    /// Every path was read.
    Whole,
}

impl Coverage {
    /// A read of the whole status that worked (`ok`) or failed.
    pub(super) fn whole(ok: bool) -> Self {
        if ok { Self::Whole } else { Self::Failed }
    }

    /// What a status read's result covered. Every whole read is all-or-nothing, so only path
    /// and index reads are partial; a read that can stop early must map here too.
    pub(super) fn of(r: &Result<StatusRead, ReadFailed>) -> Self {
        match r {
            Ok(StatusRead::Whole(..)) => Self::Whole,
            Ok(_) => Self::Partial,
            Err(_) => Self::Failed,
        }
    }
}

/// The conflicts toast just shown (the newest) and run `entry` of `job` wait for the status;
/// `named` and `unsure` are the paths git's lines gave.
pub(super) fn expect(st: &mut AppState, job: &RunningJob, entry: EntryId, named: Vec<String>, unsure: Vec<Vec<String>>) {
    let (Some(workdir), Some(toast)) = (job.workdir.clone(), st.newest_toast().map(|t| t.id)) else { return };
    st.logs.recount = Some(Recount { toast, entry, workdir, generation: st.load_gen, named, unsure, partial: false });
}

/// The active tab's status was read for load generation `st.load_gen`, covering `read`. A
/// partial read of the job's repository keeps git's count, hedged, and keeps waiting; the first
/// other read of a load started after the job ends the wait, and a complete one of the job's
/// repository that lists conflicts gives the count and the files.
pub(super) fn status_read(st: &mut AppState, read: Coverage) {
    let gen_now = st.load_gen;
    let Some(r) = st.logs.recount.as_ref().filter(|r| gen_now > r.generation) else { return };
    let same = st.snap.as_ref().is_some_and(|s| same_spelling(&s.workdir, &r.workdir));
    if read == Coverage::Partial && same {
        return hedge(st);
    }
    let Some(r) = st.logs.recount.take() else { return };
    if read != Coverage::Whole || !same {
        return; // git's count stays
    }
    let Some(files) = st.snap.as_ref().map(unmerged).filter(|f| !f.is_empty()) else { return };
    let t = st.settings.lang();
    let text = crate::state::error_words::conflicts(t, files.len());
    let up = set_text(st, r.toast, r.entry, &text);
    let shown: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let nameable = |p: &String| files.iter().any(|f| f.path == *p && !matches!(f.hazard, Some(Hazard::NotUtf8(_))));
    let msg = match ordered(&r.named, &r.unsure, &shown).into_iter().find(nameable) {
        Some(p) => Msg::Conflict(ConflictMsg::OpenConflict(p)),
        // Only names git can't be given: the WIP panel's Conflicted Files list shows them, marked.
        None => Msg::Repo(RepoMsg::SelectWip),
    };
    let view = ToastAction { label: t.tr("details.banner.view_conflicts.button").to_owned(), msg: Box::new(msg), workdir: Some(r.workdir) };
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

/// A partial read: git's count, once, as a floor ("at least"), since the read kept the
/// conflicted rows from before the job. No count when git's lines named no file.
fn hedge(st: &mut AppState) {
    let Some(r) = st.logs.recount.as_mut() else { return };
    if r.partial {
        return;
    }
    r.partial = true;
    let (n, toast, entry) = (r.named.len() + r.unsure.len(), r.toast, r.entry);
    if n == 0 {
        return;
    }
    let text = crate::state::error_words::conflicts_incomplete(st.settings.lang(), n);
    set_text(st, toast, entry, &text);
}

/// Puts `text` in toast `toast` (while it is up) and run `entry`'s summary. Whether the toast
/// is up.
fn set_text(st: &mut AppState, toast: u64, entry: EntryId, text: &str) -> bool {
    let toast = st.toasts.iter_mut().find(|x| x.id == toast);
    let up = toast.is_some();
    if let Some(x) = toast {
        text.clone_into(&mut x.text);
    }
    if let Some(run) = st.log.iter_mut().find(|e| e.id == entry).and_then(|e| e.run.as_mut()) {
        text.clone_into(&mut run.summary);
    }
    up
}

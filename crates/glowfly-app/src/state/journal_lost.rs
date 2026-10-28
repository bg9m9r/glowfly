//! The journal's Lost commits tab (pure): which tab shows, the lost list and the selected
//! commit's preview as loaded (`JournalView::lost`, parked with the view), and what each of its
//! actions does. A plain lost commit is restored through the journal's own Restore of a HEAD
//! state (a detached checkout, confirmed, journaled and undoable) or kept with a new branch; a
//! dropped stash is applied (asking first on a dirty tree; not while a job runs or an operation
//! is in progress) or put back in the stash list. A discarded file Glowfly kept is restored at
//! its path only where nothing is, or as a copy next to it (`undo::kept::restore`).

use super::journal_restore::{self, RestorePlan};
use super::palette_kinds::StashAction;
use super::stash_confirm::StashRun;
use crate::msg::{BranchMsg, JobMsg, Msg};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::journal::{Event, LostCommit, LostCommits, LostKind, Preview, Row, Subject, lost};
use glowfly_core::repo::Snapshot;
use glowfly_core::undo::kept::{KeptWhat, Restored};

/// The header's States | Lost commits control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JournalTab {
    #[default]
    States,
    Lost,
}

/// The Lost commits tab's data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LostView {
    /// The load in flight or last asked for (`RepoSession::journal`).
    pub generation: u64,
    pub loading: bool,
    /// A refresh happened while the tab was hidden or a load ran: load again when it shows.
    pub stale: bool,
    /// The last load's result; `None` until the first arrives.
    pub result: Option<Result<LostCommits, String>>,
    pub selected: Option<Oid>,
    pub preview: Option<LostPreviewState>,
}

/// The history shown for the selected lost commit.
#[derive(Clone, Debug, PartialEq)]
pub struct LostPreviewState {
    pub oid: Oid,
    pub generation: u64,
    /// `None` while it loads.
    pub result: Option<Result<Preview, String>>,
}

impl LostView {
    /// The commits of the last good load, newest first.
    pub fn commits(&self) -> &[LostCommit] {
        match &self.result {
            Some(Ok(l)) => &l.commits,
            _ => &[],
        }
    }

    /// The discarded changes Glowfly kept, newest first.
    pub fn discarded(&self) -> &[LostCommit] {
        match &self.result {
            Some(Ok(l)) => &l.discarded,
            _ => &[],
        }
    }

    /// The listed entry `oid`: a kept discard or a lost commit.
    pub fn commit(&self, oid: Oid) -> Option<&LostCommit> {
        self.discarded().iter().chain(self.commits()).find(|c| c.oid == oid)
    }

    /// Nothing is listed: no discarded changes and no lost commits.
    pub fn is_empty(&self) -> bool {
        self.discarded().is_empty() && self.commits().is_empty()
    }

    pub fn selected_commit(&self) -> Option<&LostCommit> {
        self.selected.and_then(|o| self.commit(o))
    }

    /// The list was cut at `MAX_LOST`.
    pub fn capped(&self) -> bool {
        matches!(&self.result, Some(Ok(l)) if l.capped)
    }

    /// fsck reported problems, so some may be missing.
    pub fn incomplete(&self) -> bool {
        matches!(&self.result, Some(Ok(l)) if l.incomplete)
    }
}

/// What a lost commit is called: its summary, or "(no message)".
pub fn label(t: Lang, c: &LostCommit) -> String {
    if c.summary.trim().is_empty() { t.tr("journal.lost.no_message").into() } else { glowfly_core::text::printable(&c.summary) }
}

/// Where its history starts: a dropped stash's base, else the commit itself; a kept file has
/// none.
pub fn history_from(c: &LostCommit) -> Option<Oid> {
    match &c.kind {
        LostKind::Stash { base, .. } => Some(*base),
        LostKind::Commit => Some(c.oid),
        LostKind::Discarded { .. } => None,
    }
}

/// The chip naming what was thrown away: a kept file's discard or delete, a dropped stash.
pub fn chip(t: Lang, kind: &LostKind) -> Option<&'static str> {
    Some(match kind {
        LostKind::Commit => return None,
        LostKind::Stash { .. } => t.tr("journal.lost.stash_chip"),
        LostKind::Discarded { what, .. } => match what {
            KeptWhat::File => t.tr("journal.lost.what.file"),
            KeptWhat::Hunk => t.tr("journal.lost.what.hunk"),
            KeptWhat::Lines => t.tr("journal.lost.what.lines"),
            KeptWhat::Revert => t.tr("journal.lost.what.revert"),
            KeptWhat::Delete => t.tr("journal.lost.what.delete"),
            KeptWhat::Untracked => t.tr("journal.lost.what.untracked"),
            KeptWhat::Stash => t.tr("journal.lost.stash_chip"),
        },
    })
}

/// Why a kept file can't be restored now: another job is running (it may be writing the same
/// files).
pub fn restore_file_blocked(job_running: bool, t: Lang) -> Option<&'static str> {
    job_running.then(|| t.tr("commit.blocked.job_running"))
}

/// The toast for a Restore of the kept file `path` (`ok`, its text, and whether to offer the copy).
pub fn restored_toast(t: Lang, path: &str, result: &Result<Restored, String>) -> (bool, String, bool) {
    match result {
        Ok(Restored::Wrote { path: wrote }) if wrote == path => (true, t.trf("journal.lost.restored", &[("path", &path)]), false),
        Ok(Restored::Wrote { path: wrote }) => (true, t.trf("journal.lost.restored_copy", &[("path", &path), ("copy", wrote)]), false),
        Ok(Restored::AlreadyThere { .. }) => (true, t.trf("journal.lost.already_there", &[("path", &path)]), false),
        Ok(Restored::Occupied { .. }) => (false, t.trf("journal.lost.occupied", &[("path", &path)]), true),
        Err(e) => (false, t.trf("journal.lost.restore_failed", &[("path", &path), ("error", e)]), false),
    }
}

/// The lost commit as a HEAD state, for the journal's Restore.
pub fn as_row(c: &LostCommit) -> Row {
    Row {
        subject: Subject::Head,
        oid: c.oid,
        prev: None,
        at: c.time,
        event: Event::Other,
        message: String::new(),
        note: None,
        summary: Some(c.summary.clone()),
        current: false,
        unreachable: true,
    }
}

/// What Restore… does for `c` (a detached checkout), or why it can't now.
pub fn restore_plan(snap: &Snapshot, c: &LostCommit, job_running: bool, t: Lang) -> Result<RestorePlan, String> {
    journal_restore::plan(snap, &as_row(c), job_running, t)
}

/// The confirmation that runs `plan`, stashing first on a dirty tree.
pub fn restore_confirm(t: Lang, plan: &RestorePlan, snap: &Snapshot) -> Msg {
    journal_restore::confirm(t, plan, None, super::auto_stash::blocks(snap), snap.head_branch.as_deref())
}

/// Create branch here…: the New branch dialog at the commit, unnamed.
pub fn create_branch(c: &LostCommit) -> BranchMsg {
    BranchMsg::NewBranch { start: Some(c.oid.to_string()), name: String::new() }
}

/// Why Apply stash can't run now: another job is running, or a merge, rebase or other
/// operation is in progress (its stopped state would be mixed with the stash's changes).
pub fn apply_blocked(snap: &Snapshot, job_running: bool, t: Lang) -> Option<&'static str> {
    if job_running {
        Some(t.tr("commit.blocked.job_running"))
    } else if snap.state != git2::RepositoryState::Clean {
        Some(t.tr("journal.blocked.in_progress"))
    } else {
        None
    }
}

/// Apply stash: `git stash apply <oid>`, asking first on a dirty tree.
pub fn apply(snap: &Snapshot, c: &LostCommit, t: Lang) -> Msg {
    let run = StashRun::Git { label: t.tr("journal.lost.apply.job").into(), cmds: vec![lost::apply_stash(c.oid)] };
    super::stash_confirm::guarded(snap, t, StashAction::Apply, c.oid, &c.summary, run)
}

/// Put back in stash list: `git stash store -m <summary> <oid>`.
pub fn store(c: &LostCommit, t: Lang) -> Msg {
    Msg::Job(JobMsg::Git { label: t.tr("journal.lost.store.job").into(), cmds: vec![lost::store_stash(c.oid, &c.summary)] })
}

/// The footer lines under the list: cut at `MAX_LOST`, then fsck's problems.
pub fn footer_lines(t: Lang, view: &LostView) -> Vec<&'static str> {
    let mut out = Vec::new();
    if view.capped() {
        out.push(t.tr("journal.lost.capped"));
    }
    if view.incomplete() {
        out.push(t.tr("journal.lost.incomplete"));
    }
    out
}

#[cfg(test)]
#[path = "journal_lost_tests.rs"]
mod tests;

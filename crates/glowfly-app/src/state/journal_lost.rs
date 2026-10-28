//! The journal's Lost commits tab (pure): which tab shows, the lost list and the selected
//! commit's preview as loaded (`JournalView::lost`, parked with the view), and what each of its
//! actions does. A plain lost commit is restored through the journal's own Restore of a HEAD
//! state (a detached checkout, confirmed, journaled and undoable) or kept with a new branch; a
//! dropped stash is applied (asking first on a dirty tree) or put back in the stash list.

use super::journal_restore::{self, RestorePlan};
use super::palette_kinds::StashAction;
use super::stash_confirm::StashRun;
use crate::msg::{BranchMsg, JobMsg, Msg};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::journal::{Event, LostCommit, LostCommits, LostKind, Preview, Row, Subject, lost};
use glowfly_core::repo::Snapshot;

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

    pub fn commit(&self, oid: Oid) -> Option<&LostCommit> {
        self.commits().iter().find(|c| c.oid == oid)
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

/// Where its history starts: a dropped stash's base, else the commit itself.
pub fn history_from(c: &LostCommit) -> Oid {
    match &c.kind {
        LostKind::Stash { base, .. } => *base,
        LostKind::Commit => c.oid,
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

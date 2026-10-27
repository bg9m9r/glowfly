//! The per-tab views that live with their generation counters, the open diff, the commit detail,
//! the commit message draft and what was read from the repository: the active tab's in
//! `AppState::session`, a parked tab's in `Parked::session`. Switching tabs swaps the struct
//! whole (update/tabs.rs): `park` takes the views out and leaves the counters behind, and
//! `resume_after` moves every counter of the tab coming back past both its own and the ones
//! the previous tab left. So the active counter is always at least every value any tab ever
//! handed out, and a result still in flight from before a park, or for another tab, never
//! matches.

use super::blame::BlameView;
use super::conflict::ConflictView;
use super::conflict_files::ConflictFiles;
use super::diff_preview::DiffPreview;
use super::gitflow_resume::StoppedFinish;
use super::history::FileHistoryView;
use super::journal::JournalView;
use super::range_diff::RangeDiffView;
use super::rebase::RebasePanel;
use glowfly_core::repo::{CommitDetail, DiffTarget, FileDiff};

/// A view that may be open, with the generation its requests carry: incremented for every
/// request and whenever the view closes, so results of older requests are ignored.
pub struct Gen<T> {
    pub view: Option<T>,
    pub generation: u64,
}

// By hand: a derive would require `T: Default`.
impl<T> Default for Gen<T> {
    fn default() -> Self {
        Self { view: None, generation: 0 }
    }
}

#[cfg(test)]
impl<T> Gen<T> {
    /// `view` open, at generation 0.
    pub fn open(view: T) -> Self {
        Self { view: Some(view), generation: 0 }
    }
}

/// The pull request panel with its counters, each incremented for every request of its kind
/// and whenever the panel closes.
#[derive(Default)]
pub struct PrSlot {
    /// Shown instead of a diff or the graph (below a conflict and the file history). A file
    /// list in flight is asked for again on return.
    pub view: Option<super::pull_request::PrView>,
    /// The file list's requests.
    pub generation: u64,
    /// The panel's page.
    pub page_gen: u64,
    /// The checks.
    pub checks_gen: u64,
    /// The line maps.
    pub lines_gen: u64,
}

/// The open diff with its counter.
#[derive(Default)]
pub struct DiffSlot {
    /// Shared with effects that read it (a reload, a highlight) instead of copied for them.
    pub view: Option<(DiffTarget, std::sync::Arc<FileDiff>)>,
    /// Incremented for every diff request and whenever the diff on screen goes stale (closed,
    /// replaced, or changed by a hunk operation), and when its tab comes back; in `counters()`.
    /// Results of older requests are ignored.
    pub generation: u64,
    /// The request that produced the diff on screen. Hunk operations only run while this is
    /// `generation`: otherwise a reload is on its way and the hunk indexes may no longer match.
    /// Not a counter: `park` leaves it behind and `resume_after` takes the current one, so it
    /// stays the last request answered whichever tab is active.
    pub shown_gen: u64,
    /// The open diff's LFS summary and whether it shows the pointer text instead.
    pub lfs: Option<super::lfs::LfsDiffNote>,
}

impl DiffSlot {
    /// Starts a new diff request and returns its generation: results still in flight, and hunk
    /// operations aimed at the diff on screen, are stale from now on.
    pub fn next(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// The generation hunk operations carry, while the diff on screen is the latest request's.
    pub fn hunk_ops_gen(&self) -> Option<u64> {
        (self.view.is_some() && self.shown_gen == self.generation).then_some(self.generation)
    }
}

/// The active tab's views (in `AppState::session`), or a parked tab's (in `Parked::session`).
#[derive(Default)]
pub struct RepoSession {
    /// The conflict being resolved, with its picks; shown instead of the graph or a diff. A
    /// load in flight is asked for again on return.
    pub conflict: Gen<ConflictView>,
    /// The WIP panel's Conflicted Files counts and Resolved Files: read again on return. It
    /// keeps its own generation (`conflict_files::want`).
    pub conflict_files: ConflictFiles,
    /// The open file history; shown instead of a diff or the graph (below a conflict). A
    /// request in flight is sent again on return.
    pub history: Gen<FileHistoryView>,
    /// The open blame view; shown above the file history, a diff and the panels, below a
    /// conflict. A load in flight is asked for again on return.
    pub blame: Gen<BlameView>,
    /// The pull request panel with its four counters.
    pub pull_request: PrSlot,
    /// The open issue panel; in the pull request panel's place (the two never show together).
    /// Its details load again on return.
    pub issue: Gen<super::issue::IssueView>,
    /// The interactive rebase panel; shown instead of the graph (below a diff, the file history
    /// and a conflict). A list still loading is asked for again on return.
    pub rebase: Gen<RebasePanel>,
    /// The Git Flow panel is open; shown in place of the graph (below the rebase panel).
    pub gitflow_panel: bool,
    /// A Finish stopped at a merge conflict, with its steps still to run.
    pub gitflow_resume: Option<StoppedFinish>,
    /// The operation journal, shown in place of the graph (update/journal.rs). A load or
    /// preview in flight is asked for again on return.
    pub journal: Gen<JournalView>,
    /// The Range-diff view, shown in place of the graph (update/range_diff.rs). A comparison
    /// in flight is asked for again on return.
    pub range_diff: Gen<RangeDiffView>,
    /// The binary diff on screen's sizes and image previews (update/diff_preview.rs); one still
    /// loading is asked for again on return.
    pub diff_preview: Option<DiffPreview>,
    /// The open diff, its counter and LFS note.
    pub diff: DiffSlot,
    /// The selected commit's details, shown in the right panel.
    pub detail: Option<CommitDetail>,
    /// The commit message draft. Views edit it in place as a text buffer.
    pub draft: super::CommitDraft,
    /// What was read from the tab's repository: commit stats, line counts, signatures and the
    /// last conflict check.
    pub reads: super::session_reads::RepoReads,
}

impl RepoSession {
    /// Every generation counter, in one list: `park` and `resume_after` go through it (add a
    /// new pair here). The last three are the repository reads' (`RepoReads::counters`).
    fn counters(&mut self) -> [&mut u64; 15] {
        let [wip, sigs, checks] = self.reads.counters();
        let pr = &mut self.pull_request;
        [
            &mut self.conflict.generation,
            &mut self.history.generation,
            &mut self.blame.generation,
            &mut pr.generation,
            &mut pr.page_gen,
            &mut pr.checks_gen,
            &mut pr.lines_gen,
            &mut self.issue.generation,
            &mut self.rebase.generation,
            &mut self.journal.generation,
            &mut self.range_diff.generation,
            &mut self.diff.generation,
            wip,
            sigs,
            checks,
        ]
    }

    /// Takes the session out to park with its tab. What stays behind has no views but the same
    /// counters, so the next tab's counters move past them even when this tab is closed rather
    /// than parked; the diff's `shown_gen` stays behind too. A conflict save in flight reports
    /// only to the active tab, so the parked view doesn't wait for it. The repository reads'
    /// results go with the tab and their requests in flight are dropped; what stays behind keeps
    /// the reads' counters, moved past those requests (`RepoReads::parked`).
    pub fn park(&mut self) -> RepoSession {
        let results = self.reads.parked();
        let mut parked = std::mem::take(self);
        self.reads = std::mem::replace(&mut parked.reads, results);
        for (left, taken) in self.counters().into_iter().zip(parked.counters()) {
            *left = *taken;
        }
        self.diff.shown_gen = parked.diff.shown_gen;
        if let Some(c) = parked.conflict.view.as_mut() {
            c.saving = false;
        }
        parked
    }

    /// This parked session becomes active after `current` (what `park` left behind): every
    /// counter moves past both its own value and `current`'s, so a result still in flight from
    /// before the park, or for another tab, never matches. The diff's `shown_gen` is taken from
    /// `current`, so the open diff's hunks stay off until the refresh after the switch reloads it.
    pub fn resume_after(&mut self, current: &mut RepoSession) {
        for (mine, now) in self.counters().into_iter().zip(current.counters()) {
            *mine = (*mine).max(*now) + 1;
        }
        self.diff.shown_gen = current.diff.shown_gen;
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

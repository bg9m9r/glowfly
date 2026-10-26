//! The per-tab views that live with their generation counters: the active tab's in
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
}

impl RepoSession {
    /// Every generation counter, in one list: `park` and `resume_after` go through it (add a
    /// new pair here).
    fn counters(&mut self) -> [&mut u64; 11] {
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
        ]
    }

    /// Takes the session out to park with its tab. What stays behind has no views but the same
    /// counters, so the next tab's counters move past them even when this tab is closed rather
    /// than parked. A conflict save in flight reports only to the active tab, so the parked
    /// view doesn't wait for it.
    pub fn park(&mut self) -> RepoSession {
        let mut parked = std::mem::take(self);
        for (left, taken) in self.counters().into_iter().zip(parked.counters()) {
            *left = *taken;
        }
        if let Some(c) = parked.conflict.view.as_mut() {
            c.saving = false;
        }
        parked
    }

    /// This parked session becomes active after `current` (what `park` left behind): every
    /// counter moves past both its own value and `current`'s, so a result still in flight from
    /// before the park, or for another tab, never matches.
    pub fn resume_after(&mut self, current: &mut RepoSession) {
        for (mine, now) in self.counters().into_iter().zip(current.counters()) {
            *mine = (*mine).max(*now) + 1;
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

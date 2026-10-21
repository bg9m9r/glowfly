//! Messages of the interactive rebase panel, the graph's history edits and the rebase banner
//! (update/rebase.rs, update/rebase_remaining.rs, update/rebase_quick.rs,
//! update/rebase_resume.rs, update/stack.rs for restacks).

use crate::effect::{RestackJob, UndoResult};
use crate::state::rebase_steps::NewStep;
use glowfly_core::rebase::{Outcome, QuickEdit, QuickPlan, RebaseAction, RebaseBase, RebasePlan, Remaining, Resume};
use glowfly_core::stack::{RestackRun, RestackScope};
use std::path::PathBuf;

/// The interactive rebase panel: editing the list, starting the rebase, and the results
/// (update/rebase.rs); editing the remaining steps of a running rebase
/// (update/rebase_remaining.rs); history edits from the graph's menus (update/rebase_quick.rs);
/// Continue and Skip in the rebase banner (update/rebase_resume.rs).
pub enum RebaseMsg {
    /// Open the panel with the list for rebasing the current branch from `base` (from a commit,
    /// or onto a branch or a commit).
    OpenRebase(RebaseBase),
    CloseRebase,
    RebaseSetAction {
        index: usize,
        action: RebaseAction,
    },
    /// Move row `from` so it ends up at index `to` (removed, then inserted).
    RebaseMove {
        from: usize,
        to: usize,
    },
    /// Select row `i` (a click or the arrow keys); a commit row also shows its commit.
    RebaseSelect(usize),
    /// Add an exec or break step after row `after` (its group's last row when it is in a
    /// squash group), or at the end.
    RebaseAddStep {
        after: Option<usize>,
        step: NewStep,
    },
    /// Remove row `i`, an exec or break step.
    RebaseRemoveStep(usize),
    /// Open row `i`'s editor: its message, its group's combined message, or its command.
    RebaseEditMessage(usize),
    /// The open editor's save button: a message makes the row a reword, a combined message or
    /// a command is stored.
    RebaseSaveMessage,
    RebaseCancelMessage,
    /// "Use git's message" in the combined-message editor: forget the typed one.
    RebaseResetCombined,
    /// The original order and actions (or, for a stale list, the list reloaded).
    RebaseReset,
    /// Start the rebase (or update the remaining steps); drops and uncommitted changes ask
    /// first (`confirmed: false`). `autostash`: the question mentioned stashing, so tracked
    /// changes are stashed first.
    StartRebase {
        confirmed: bool,
        autostash: bool,
    },
    RebasePlanLoaded {
        generation: u64,
        result: Result<RebasePlan, String>,
    },
    /// Open the panel on the remaining steps of the interactive rebase in progress.
    OpenRemaining,
    RemainingLoaded {
        generation: u64,
        result: Result<Remaining, String>,
    },
    /// The edit-todo job's result, for whichever tab it ran in (like `JobDone`).
    RemainingEdited {
        result: Result<(), String>,
    },
    /// The rebase job's result, for whichever tab it ran in (like `JobDone`), with what it
    /// means for the undo history.
    RebaseFinished {
        result: Result<Outcome, String>,
        undo: Option<UndoResult>,
    },
    /// A history edit chosen in the graph's menus: prepare its rebase.
    Edit(QuickEdit),
    /// The prepared edit, for the tab at `workdir`.
    EditPrepared {
        workdir: PathBuf,
        result: Result<QuickPlan, String>,
    },
    /// The Reword or Squash dialog's primary button; the form is read from `st.modal`.
    SubmitReword,
    /// Run a prepared edit. `confirmed`: the user was asked; `autostash`: that question
    /// mentioned stashing the uncommitted changes.
    RunEdit {
        plan: Box<QuickPlan>,
        autostash: bool,
        confirmed: bool,
    },
    /// The rebase banner's Continue or Skip commit.
    Resume(Resume),
    /// Restack the branches in `scope` (the branch menu's stack items, the offer after a
    /// rewrite; update/stack.rs). Uncommitted changes ask first; `autostash`: that question was
    /// answered, so each step stashes them.
    Restack {
        scope: RestackScope,
        autostash: bool,
    },
    /// A restack job's result (its first run, or the banner's Continue or Skip of a stopped
    /// one), for whichever tab it ran in (like `RebaseFinished`).
    RestackRan {
        result: Result<RestackRun, String>,
        undo: Option<UndoResult>,
        job: Box<RestackJob>,
    },
}

impl RebaseMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::OpenRebase(..)
            | Self::CloseRebase
            | Self::RebaseSetAction { .. }
            | Self::RebaseMove { .. }
            | Self::RebaseSelect(..)
            | Self::RebaseAddStep { .. }
            | Self::RebaseRemoveStep(..)
            | Self::RebaseEditMessage(..)
            | Self::RebaseSaveMessage
            | Self::RebaseCancelMessage
            | Self::RebaseResetCombined
            | Self::RebaseReset
            | Self::StartRebase { .. }
            | Self::RebasePlanLoaded { .. }
            | Self::OpenRemaining
            | Self::RemainingLoaded { .. }
            | Self::RemainingEdited { .. }
            | Self::RebaseFinished { .. }
            | Self::Edit(..)
            | Self::EditPrepared { .. }
            | Self::SubmitReword
            | Self::RunEdit { .. }
            | Self::Resume(..)
            | Self::Restack { .. }
            | Self::RestackRan { .. } => None,
        }
    }
}

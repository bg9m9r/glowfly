//! Messages of the interactive rebase panel, the graph's history edits and the rebase banner
//! (update/rebase.rs, update/rebase_quick.rs, update/rebase_resume.rs).

use std::path::PathBuf;
use trellis_core::rebase::{Outcome, QuickEdit, QuickPlan, RebaseAction, RebaseBase, RebasePlan, Resume};

/// The interactive rebase panel: editing the list, starting the rebase, and the results
/// (update/rebase.rs); history edits from the graph's menus (update/rebase_quick.rs); Continue
/// and Skip in the rebase banner (update/rebase_resume.rs).
pub enum RebaseMsg {
    /// Open the panel with the list for rebasing the current branch from `base`.
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
    /// Open the message editor on row `i`.
    RebaseEditMessage(usize),
    /// Update message: keep the editor's text and make the row a reword.
    RebaseSaveMessage,
    RebaseCancelMessage,
    /// The original order and actions (or, for a stale list, the list reloaded).
    RebaseReset,
    /// Start the rebase; drops and uncommitted changes ask first (`confirmed: false`).
    /// `autostash`: the question mentioned stashing, so tracked changes are stashed first.
    StartRebase {
        confirmed: bool,
        autostash: bool,
    },
    RebasePlanLoaded {
        generation: u64,
        result: Result<RebasePlan, String>,
    },
    /// The rebase job's result, for whichever tab it ran in (like `JobDone`).
    RebaseFinished {
        result: Result<Outcome, String>,
    },
    /// A history edit chosen in the graph's menus: prepare its rebase.
    Edit(QuickEdit),
    /// The prepared edit, for the tab at `workdir`.
    EditPrepared {
        workdir: PathBuf,
        result: Result<QuickPlan, String>,
    },
    /// The Reword dialog's primary button; the form is read from `st.modal`.
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
            | Self::RebaseEditMessage(..)
            | Self::RebaseSaveMessage
            | Self::RebaseCancelMessage
            | Self::RebaseReset
            | Self::StartRebase { .. }
            | Self::RebasePlanLoaded { .. }
            | Self::RebaseFinished { .. }
            | Self::Edit(..)
            | Self::EditPrepared { .. }
            | Self::SubmitReword
            | Self::RunEdit { .. }
            | Self::Resume(..) => None,
        }
    }
}

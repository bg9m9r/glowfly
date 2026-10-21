//! Interactive rebase effects: loading the panel's list, preparing a history edit chosen in the
//! graph, running a rebase, resuming a stopped one, and reading and editing the remaining steps
//! of a running one; restacking stacked branches (`git rebase --update-refs`) and going on with
//! a stopped restack.

use glowfly_core::rebase::{EditTodoRequest, QuickEdit, RebaseBase, Resume, StartRequest};
use glowfly_core::stack::{RestackPlan, RestackStep};
use glowfly_core::undo::RestackBefore;
use std::path::PathBuf;

/// A restack job: the steps still to run, every branch's tip before its first run (for undo),
/// how the undo history names it ("restack of the branches above search"), and the step a
/// stopped run stopped in.
#[derive(Debug, Clone)]
pub struct RestackJob {
    pub plan: RestackPlan,
    pub before: Option<RestackBefore>,
    pub detail: String,
    pub stopped: Option<RestackStep>,
}

#[derive(Debug)]
pub enum RebaseEffect {
    /// The commits an interactive rebase from `base` would replay (`RebaseMsg::RebasePlanLoaded`).
    LoadPlan { workdir: PathBuf, base: RebaseBase, generation: u64 },
    /// Runs an interactive rebase with a prepared todo list. Unscoped like `RunGit`: the job's
    /// result must reach `update` even if its tab is no longer active. `detail` names it in the
    /// undo history ("reword of 1a2b3c4").
    Start { cwd: PathBuf, request: StartRequest, detail: String },
    /// The rebase that makes a graph edit (`RebaseMsg::EditPrepared`).
    PrepareEdit { workdir: PathBuf, edit: QuickEdit },
    /// Continues or skips the rebase in progress; a job, unscoped like `Start`.
    Resume { cwd: PathBuf, how: Resume },
    /// The remaining steps of the interactive rebase in progress (`RebaseMsg::RemainingLoaded`).
    LoadRemaining { workdir: PathBuf, generation: u64 },
    /// Replaces the remaining steps (`git rebase --edit-todo`); a job, unscoped like `Start`.
    EditTodo { cwd: PathBuf, request: EditTodoRequest },
    /// Runs a restack's steps (`RebaseMsg::RestackRan`); a job, unscoped like `Start`.
    Restack { cwd: PathBuf, job: RestackJob },
    /// Continues or skips the step a restack stopped at, then runs the steps it has left; a job,
    /// unscoped like `Start`.
    ResumeRestack { cwd: PathBuf, how: Resume, job: RestackJob },
}

impl RebaseEffect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoadPlan { .. } => "rebase-plan",
            Self::Start { .. } => "rebase",
            Self::PrepareEdit { .. } => "rebase-edit",
            Self::Resume { .. } => "rebase-resume",
            Self::LoadRemaining { .. } => "rebase-remaining",
            Self::EditTodo { .. } => "rebase-edit-todo",
            Self::Restack { .. } => "restack",
            Self::ResumeRestack { .. } => "restack-resume",
        }
    }

    /// The tab the result belongs to: the reads answer their tab; the jobs end the active job
    /// whichever tab is active.
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            Self::LoadPlan { workdir, .. } | Self::PrepareEdit { workdir, .. } | Self::LoadRemaining { workdir, .. } => Some(workdir),
            Self::Start { .. } | Self::Resume { .. } | Self::EditTodo { .. } | Self::Restack { .. } | Self::ResumeRestack { .. } => None,
        }
    }
}

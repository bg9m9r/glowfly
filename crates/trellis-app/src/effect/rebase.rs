//! Interactive rebase effects: loading the panel's list, preparing a history edit chosen in the
//! graph, running a rebase, and resuming a stopped one.

use std::path::PathBuf;
use trellis_core::rebase::{QuickEdit, RebaseBase, Resume, StartRequest};

#[derive(Debug)]
pub enum RebaseEffect {
    /// The commits an interactive rebase from `base` would replay (`RebaseMsg::RebasePlanLoaded`).
    LoadPlan { workdir: PathBuf, base: RebaseBase, generation: u64 },
    /// Runs an interactive rebase with a prepared todo list. Unscoped like `RunGit`: the job's
    /// result must reach `update` even if its tab is no longer active.
    Start { cwd: PathBuf, request: StartRequest },
    /// The rebase that makes a graph edit (`RebaseMsg::EditPrepared`).
    PrepareEdit { workdir: PathBuf, edit: QuickEdit },
    /// Continues or skips the rebase in progress; a job, unscoped like `Start`.
    Resume { cwd: PathBuf, how: Resume },
}

impl RebaseEffect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoadPlan { .. } => "rebase-plan",
            Self::Start { .. } => "rebase",
            Self::PrepareEdit { .. } => "rebase-edit",
            Self::Resume { .. } => "rebase-resume",
        }
    }

    /// The tab the result belongs to: the reads answer their tab; the jobs end the active job
    /// whichever tab is active.
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            Self::LoadPlan { workdir, .. } | Self::PrepareEdit { workdir, .. } => Some(workdir),
            Self::Start { .. } | Self::Resume { .. } => None,
        }
    }
}

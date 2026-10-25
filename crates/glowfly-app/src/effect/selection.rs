//! The selection's effects (`Effect::Selection`): comparing two commits or a commit and the
//! working tree, and each selected commit's own changes for a multi-selection with no combined
//! diff (runner/selection.rs).

#[cfg(test)]
use super::SandboxRule;
use super::meta::EffectMeta;
use git2::Oid;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum SelectionEffect {
    /// The files that differ from commit `from` to commit `to`.
    Compare { workdir: PathBuf, from: Oid, to: Oid },
    /// The files that differ from commit `oid` to the working tree.
    WorkdirCompare {
        workdir: PathBuf,
        oid: Oid,
        generation: u64,
        /// From the working tree to the commit instead.
        swapped: bool,
    },
    /// Each of these commits' own changes (`SelectionMsg::StackedLoaded`).
    Stacked(StackRequest),
}

#[derive(Debug)]
pub struct StackRequest {
    pub workdir: PathBuf,
    /// The commits, newest first (the selection's `stack_oids`).
    pub oids: Vec<Oid>,
}

impl EffectMeta for SelectionEffect {
    fn name(&self) -> &'static str {
        match self {
            SelectionEffect::Compare { .. } => "load-compare",
            SelectionEffect::WorkdirCompare { .. } => "load-workdir-compare",
            SelectionEffect::Stacked(_) => "load-stacked",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            SelectionEffect::Compare { workdir, .. } | SelectionEffect::WorkdirCompare { workdir, .. } => Some(workdir),
            SelectionEffect::Stacked(r) => Some(&r.workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// All read.
    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

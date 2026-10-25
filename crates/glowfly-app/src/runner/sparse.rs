//! Sparse checkout for the preferences page: reading the worktree's state
//! (`SparseEffect::Load`) and running `git sparse-checkout` set, reapply or disable
//! (`SparseEffect::Checkout`).

use super::execute::{Cx, Execute};
use crate::effect::SparseEffect;
use crate::msg::{Msg, SparseMsg};
use glowfly_core::sparse_ops::{self, SparseCommand};
use std::path::Path;

impl Execute for SparseEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            SparseEffect::Load { workdir, generation, run_git } => load(&workdir, generation, run_git),
            SparseEffect::Checkout { cwd, command } => run(&cwd, &command),
        })
    }
}

/// Reads the status; `run_git` false leaves the paths unread (git is held there).
pub(super) fn load(workdir: &Path, generation: u64, run_git: bool) -> Msg {
    Msg::Sparse(SparseMsg::Loaded { generation, result: sparse_ops::status(workdir, run_git).map_err(|e| e.to_string()) })
}

pub(super) fn run(cwd: &Path, command: &SparseCommand) -> Msg {
    let (ok, output) = sparse_ops::run(cwd, command);
    Msg::Sparse(SparseMsg::Done { ok, output })
}

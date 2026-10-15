//! Sparse checkout for the preferences page: reading the worktree's state
//! (`Effect::LoadSparse`) and running `git sparse-checkout` set, reapply or disable
//! (`Effect::SparseCheckout`).

use crate::msg::{Msg, SparseMsg};
use std::path::Path;
use trellis_core::sparse_ops::{self, SparseCommand};

pub(super) fn load(workdir: &Path, generation: u64) -> Msg {
    Msg::Sparse(SparseMsg::Loaded { generation, result: sparse_ops::status(workdir).map_err(|e| e.to_string()) })
}

pub(super) fn run(cwd: &Path, command: &SparseCommand) -> Msg {
    let (ok, output) = sparse_ops::run(cwd, command);
    Msg::Sparse(SparseMsg::Done { ok, output })
}

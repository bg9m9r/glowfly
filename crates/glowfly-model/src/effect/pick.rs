//! The payload of `Effect::Pick`: a cherry-pick or revert job, carried past the commits that
//! change nothing (runner/pick.rs).

use super::EffectMeta;
use super::SandboxRule;
use git2::Oid;
use glowfly_core::journal::Op;
use glowfly_core::pick_ops::Picking;
use glowfly_core::undo::UndoKind;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PickJob {
    pub cwd: PathBuf,
    /// The job's commands: an auto-stash push first when it stashes, the cherry-pick or revert
    /// last.
    pub cmds: Vec<Vec<String>>,
    pub record: Option<UndoKind>,
    pub journal: Option<Op>,
    pub prompts: bool,
    pub picking: Picking,
    /// The commits the last command takes, in its order.
    pub oids: Vec<Oid>,
    /// git has `--skip` (2.23 or newer).
    pub can_skip: bool,
}

impl EffectMeta for PickJob {
    fn name(&self) -> &'static str {
        "pick"
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        Some(&self.cwd)
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

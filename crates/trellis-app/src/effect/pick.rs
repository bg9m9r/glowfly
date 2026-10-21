//! The payload of `Effect::Pick`: a cherry-pick or revert job, carried past the commits that
//! change nothing (runner/pick.rs).

use git2::Oid;
use std::path::PathBuf;
use trellis_core::journal::Op;
use trellis_core::pick_ops::Picking;
use trellis_core::undo::UndoKind;

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

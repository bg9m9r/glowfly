//! The `Execute` impl of `LockEffect`: checking and deleting a lock file git left behind
//! (`glowfly_core::lock_file`, which refuses unless no git process runs).

use super::execute::{Cx, Execute};
use crate::effect::LockEffect;
use crate::msg::{LockMsg, Msg};
use glowfly_core::lock_file;

impl Execute for LockEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Lock(match self {
            LockEffect::Check { workdir, lock, entry } => {
                let result = lock_file::check(&workdir, &lock);
                LockMsg::Checked { entry, workdir, lock, result }
            }
            LockEffect::Delete { workdir, lock, retry } => {
                let result = lock_file::delete(&workdir, &lock);
                LockMsg::Deleted { lock, retry, result }
            }
        }))
    }
}

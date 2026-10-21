//! Reverting part of a commit or comparison diff in the working tree (`Effect::Revert`), with
//! the working copy saved before and after so the revert can be undone.

use crate::msg::{Msg, RevertWhat, StageMsg};
use glowfly_core::repo::{self, RevertRequest};
use glowfly_core::undo;
use std::path::Path;

/// Runs the revert and reports it under the file's working-tree name, the file that changes.
pub(super) fn run(workdir: &Path, req: RevertRequest) -> Msg {
    let path = req.dest.clone().unwrap_or_else(|| req.target.path().to_owned());
    let what = RevertWhat::of(&req.part);
    let (result, undo) = match undo::revert_saving(workdir, &path, what.saved_as(), || repo::revert(workdir, &req)) {
        Ok((r, a)) => (Ok(r), a),
        Err(e) => (Err(super::err(e)), None),
    };
    Msg::Stage(StageMsg::Reverted { path, what, result, undo })
}

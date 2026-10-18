//! Reverting part of a commit or comparison diff in the working tree (`Effect::Revert`).

use crate::msg::{Msg, RevertWhat, StageMsg};
use std::path::Path;
use trellis_core::repo::{self, RevertRequest};

/// Runs the revert and reports it under the file's working-tree name.
pub(super) fn run(workdir: &Path, req: RevertRequest) -> Msg {
    let path = req.dest.clone().unwrap_or_else(|| req.target.path().to_owned());
    let what = RevertWhat::of(&req.part);
    let result = repo::revert(workdir, &req).map_err(super::err);
    Msg::Stage(StageMsg::Reverted { path, what, result })
}

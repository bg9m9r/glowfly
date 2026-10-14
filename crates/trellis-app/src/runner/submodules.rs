//! Reading one submodule again before its update is confirmed (`Effect::CheckSubmodule`).

use crate::msg::Msg;
use crate::submodule::SubmoduleMsg;
use std::path::Path;
use trellis_core::repo;

/// The submodule at `path` in the repository at `workdir`, as `SubmoduleMsg::Checked`.
pub(super) fn check(workdir: &Path, path: String) -> Msg {
    let result = repo::submodules_at(workdir).map(|all| all.into_iter().find(|m| m.path == path)).map_err(|e| e.to_string());
    Msg::Submodule(SubmoduleMsg::Checked { path, result })
}

//! Copy hunk as patch's read (`Effect::CopyHunkPatch`): `repo::hunk_patch` on the repository.

use super::execute::{Cx, Execute};
use crate::effect::HunkPatchRequest;
use crate::msg::{DiffMsg, Msg};
use glowfly_core::repo;

impl Execute for HunkPatchRequest {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        let result =
            repo::hunk_patch(&self.workdir, &self.target, self.index, &self.expected, self.ignore_whitespace).map_err(|e| e.to_string());
        Some(Msg::Diff(DiffMsg::HunkPatchRead { result }))
    }
}

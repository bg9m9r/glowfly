//! Copy hunk as patch's read (`Effect::CopyHunkPatch`): `repo::hunk_patch` on the repository.

use super::execute::{Cx, Execute};
use glowfly_core::repo;
use glowfly_model::effect::HunkPatchRequest;
use glowfly_model::msg::{DiffMsg, Msg};

impl Execute for HunkPatchRequest {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        let result = repo::hunk_patch(&self.workdir, &self.target, self.index, &self.expected, self.ignore_whitespace);
        Some(Msg::Diff(DiffMsg::HunkPatchRead { result }))
    }
}

//! Finding git and its version (at startup), and listing a folder in `safe.directory`
//! (`Effect::GitEnv`).

use crate::effect::LoadKind;
use crate::git_env::{GitEnvEffect, GitEnvMsg};
use crate::msg::Msg;
use std::path::PathBuf;

pub(super) fn run(effect: GitEnvEffect) -> Msg {
    Msg::GitEnv(match effect {
        GitEnvEffect::Detect { cwd } => GitEnvMsg::Detected(trellis_core::git_version::detect(&cwd)),
        GitEnvEffect::AddSafeDirectory(root) => {
            let result = trellis_core::safe_directory::add(&root).map_err(|e| e.to_string());
            GitEnvMsg::SafeDirectoryAdded { root, result }
        }
    })
}

/// The answer to a load git refused because another user owns the repository at `root`.
pub(super) fn not_owned(generation: u64, kind: LoadKind, root: PathBuf) -> Msg {
    Msg::GitEnv(GitEnvMsg::NotOwned { generation, kind, root })
}

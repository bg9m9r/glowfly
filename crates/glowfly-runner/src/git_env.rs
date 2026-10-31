//! Finding git and its version (at startup) and whether it has the file system monitor daemon,
//! finding the installed gits and switching to one (Preferences > Git), and listing a folder in
//! `safe.directory` (`Effect::GitEnv`).

use super::execute::{Cx, Execute};
use crate::effect::LoadKind;
use crate::msg::Msg;
use glowfly_core::git_program;
use glowfly_core::git_version::{self, GitInfo};
use glowfly_model::git_env::{GitEnvEffect, GitEnvMsg};
use std::path::PathBuf;

pub(super) fn run(effect: GitEnvEffect) -> Msg {
    Msg::GitEnv(match effect {
        GitEnvEffect::Detect { generation, cwd } => GitEnvMsg::Detected { generation, info: git_version::detect(&cwd) },
        GitEnvEffect::AddSafeDirectory(root) => {
            let result = glowfly_core::safe_directory::add(&root).map_err(|e| e.to_string());
            GitEnvMsg::SafeDirectoryAdded { root, result }
        }
        GitEnvEffect::Discover { generation, cwd } => GitEnvMsg::Discovered { generation, candidates: git_program::discover(&cwd) },
        GitEnvEffect::CheckPathGit { cwd } => GitEnvMsg::PathGitChecked(git_program::path_git(&cwd)),
        GitEnvEffect::ProbeMonitor { generation, cwd } => {
            GitEnvMsg::MonitorProbed { generation, daemon: glowfly_core::repo::daemon_built(&cwd) }
        }
        // Checked before it is put into effect: a git that doesn't work is never run for a job.
        GitEnvEffect::Use { program: Some(p), cwd } => match git_program::probe(&p, &cwd) {
            Ok(v) => {
                git_program::set(Some(&p));
                let info = GitInfo { path: Some(p.clone()), version: Some(v), chosen: true };
                GitEnvMsg::Switched { program: Some(p), result: Ok(info) }
            }
            Err(r) => GitEnvMsg::Switched { program: Some(p), result: Err(r.text()) },
        },
        // Always accepted: with no git on PATH, the banner explains.
        GitEnvEffect::Use { program: None, cwd } => {
            git_program::set(None);
            GitEnvMsg::Switched { program: None, result: Ok(git_version::detect(&cwd)) }
        }
        GitEnvEffect::PickProgram => {
            let mut dialog = rfd::FileDialog::new().set_title("Choose the git program");
            if cfg!(windows) {
                dialog = dialog.add_filter("Programs", &["exe", "cmd", "bat", "com"]).add_filter("All files", &["*"]);
            }
            GitEnvMsg::Picked(dialog.pick_file())
        }
    })
}

impl Execute for GitEnvEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}

/// The answer to a load git refused because another user owns the repository at `root`.
pub(super) fn not_owned(generation: u64, kind: LoadKind, root: PathBuf) -> Msg {
    Msg::GitEnv(GitEnvMsg::NotOwned { generation, kind, root })
}

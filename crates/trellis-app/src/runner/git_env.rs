//! Finding git and its version (at startup), finding the installed gits and switching to one
//! (Preferences > Git), and listing a folder in `safe.directory` (`Effect::GitEnv`).

use crate::effect::LoadKind;
use crate::git_env::{GitEnvEffect, GitEnvMsg};
use crate::msg::Msg;
use std::path::PathBuf;
use trellis_core::git_program;
use trellis_core::git_version::{self, GitInfo};

pub(super) fn run(effect: GitEnvEffect) -> Msg {
    Msg::GitEnv(match effect {
        GitEnvEffect::Detect { cwd } => GitEnvMsg::Detected(git_version::detect(&cwd)),
        GitEnvEffect::AddSafeDirectory(root) => {
            let result = trellis_core::safe_directory::add(&root).map_err(|e| e.to_string());
            GitEnvMsg::SafeDirectoryAdded { root, result }
        }
        GitEnvEffect::Discover { generation, cwd } => GitEnvMsg::Discovered { generation, candidates: git_program::discover(&cwd) },
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

/// The answer to a load git refused because another user owns the repository at `root`.
pub(super) fn not_owned(generation: u64, kind: LoadKind, root: PathBuf) -> Msg {
    Msg::GitEnv(GitEnvMsg::NotOwned { generation, kind, root })
}

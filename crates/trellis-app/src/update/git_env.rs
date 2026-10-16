//! The git Trellis runs (the startup check, the banner, features turned off for an old git) and
//! repositories git refuses because another user owns them (explained, with an offer to list
//! them in `safe.directory`).

use crate::effect::Effect;
use crate::git_env::{GitEnvEffect, GitEnvMsg, not_owned_text};
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, Modal};
use std::path::Path;
use std::time::Instant;
use trellis_core::git_version::Feature;

pub(super) fn handle(st: &mut AppState, msg: GitEnvMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        GitEnvMsg::Detect => fx.push(Effect::GitEnv(GitEnvEffect::Detect { cwd: st.any_dir() })),
        GitEnvMsg::Detected(info) => {
            if let Some(text) = info.banner() {
                st.log_quiet(false, text);
            }
            st.git.info = Some(info);
        }
        GitEnvMsg::DismissBanner => st.git.banner_dismissed = true,
        GitEnvMsg::NotOwned { generation, kind, root } => {
            let current = generation == st.load_gen;
            super::repo::handle(st, RepoMsg::RepoLoaded { generation, kind, result: Err(not_owned_text(&root)) }, now, fx);
            if current && st.modal.is_none() {
                st.modal = Some(owner_prompt(&root));
            }
        }
        GitEnvMsg::AddSafeDirectory(root) => fx.push(Effect::GitEnv(GitEnvEffect::AddSafeDirectory(root))),
        GitEnvMsg::SafeDirectoryAdded { root, result: Ok(()) } => {
            let value = trellis_core::safe_directory::config_value(&root, cfg!(windows));
            st.log(true, format!("Added {value} to safe.directory in your global git config"));
            super::repo::handle(st, RepoMsg::Open(root), now, fx);
        }
        GitEnvMsg::SafeDirectoryAdded { root, result: Err(e) } => {
            let text = format!("Could not add {} to safe.directory: {e}", trellis_core::native_path::shown(&root));
            st.toast(false, text.clone(), now);
            st.log(false, text);
        }
    }
}

/// The question for a repository git refuses because another user owns it.
pub(super) fn owner_prompt(root: &Path) -> Modal {
    let shown = trellis_core::native_path::shown(root);
    let value = trellis_core::safe_directory::config_value(root, cfg!(windows));
    Modal::Confirm {
        title: "Repository owned by another user".into(),
        body: format!(
            "Git won't open {shown} because the folder belongs to another user account. This is usual for folders on a \
             network share or in WSL, on a FAT or exFAT drive, or copied from another computer. Git refuses them because a \
             repository's own settings can make git run programs.\n\nIf you trust this folder, Trellis can list it in \
             safe.directory in your global git config (as {value}) and open it."
        ),
        button: "Trust and open".into(),
        then: Box::new(Msg::GitEnv(GitEnvMsg::AddSafeDirectory(root.to_path_buf()))),
    }
}

/// Whether git can do `f`. When it can't, says why in the log and a toast, and returns false.
pub(super) fn allows(st: &mut AppState, f: Feature, now: Instant) -> bool {
    match st.git.blocked(f) {
        None => true,
        Some(why) => {
            st.toast(false, why.clone(), now);
            st.log(false, why);
            false
        }
    }
}

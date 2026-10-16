//! The git Trellis runs (the startup check, the banner, features turned off for an old git,
//! choosing another git in Preferences > Git) and repositories git refuses because another user
//! owns them (explained, with an offer to list them in `safe.directory`).

use crate::effect::Effect;
use crate::git_env::{GitEnvEffect, GitEnvMsg, not_owned_text};
use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::state::git_env::Checking;
use crate::state::{AppState, Modal, PrefPage};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::git_program::Refusal;
use trellis_core::git_version::{Feature, GitInfo, show};
use trellis_core::native_path::shown;

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
        GitEnvMsg::ShowPage => match st.prefs.as_mut() {
            None => super::prefs::handle(st, PrefsMsg::OpenPrefs(PrefPage::Git), now, fx),
            Some(p) => {
                p.page = PrefPage::Git;
                discover(st, fx);
            }
        },
        GitEnvMsg::Rescan => discover(st, fx),
        GitEnvMsg::Discovered { generation, candidates } => {
            if generation == st.git.page.generation {
                st.git.page.candidates = Some(candidates);
            }
        }
        GitEnvMsg::Choose(program) => choose(st, program, fx),
        GitEnvMsg::Browse => {
            if st.git.page.checking.is_none() {
                fx.push(Effect::GitEnv(GitEnvEffect::PickProgram));
            }
        }
        GitEnvMsg::Picked(None) => {}
        GitEnvMsg::Picked(Some(p)) => {
            if p.to_str().is_some() {
                choose(st, Some(p), fx);
            } else {
                refused(st, &p, Refusal::NotUnicode.text(), now);
            }
        }
        GitEnvMsg::Switched { program, result } => {
            if st.git.page.checking.as_ref().is_none_or(|c| c.program != program) {
                return;
            }
            st.git.page.checking = None;
            match result {
                Ok(info) => switched(st, program, info, now, fx),
                Err(why) => match program {
                    Some(p) => refused(st, &p, why, now),
                    None => refused_text(st, "git from your PATH".to_owned(), why, now),
                },
            }
        }
    }
}

/// Starts a scan for installed gits (Preferences > Git opened or Look again).
pub(super) fn discover(st: &mut AppState, fx: &mut Vec<Effect>) {
    let page = &mut st.git.page;
    page.generation += 1;
    page.candidates = None;
    page.refusal = None;
    fx.push(Effect::GitEnv(GitEnvEffect::Discover { generation: page.generation, cwd: AppState::outside_dir() }));
}

/// Checks `program` (None: git from PATH) before it is used; one check at a time.
fn choose(st: &mut AppState, program: Option<PathBuf>, fx: &mut Vec<Effect>) {
    let page = &mut st.git.page;
    if page.checking.is_some() {
        return;
    }
    page.refusal = None;
    page.checking = Some(Checking { program: program.clone() });
    fx.push(Effect::GitEnv(GitEnvEffect::Use { program, cwd: AppState::outside_dir() }));
}

/// The runner has put `program` into effect: save it and follow the new git everywhere.
fn switched(st: &mut AppState, program: Option<PathBuf>, info: GitInfo, now: Instant, fx: &mut Vec<Effect>) {
    // A picked path was refused unless it is valid Unicode, so nothing is lost here.
    st.settings.git_program = program.map(|p| p.to_string_lossy().into_owned());
    fx.push(super::prefs::save_settings(st));
    if let Some(text) = info.banner() {
        st.log_quiet(false, text);
    }
    let what = match (info.version, info.path.as_deref()) {
        (Some(v), Some(p)) => format!("git {} at {}", show(v), shown(p)),
        (Some(v), None) => format!("git {}", show(v)),
        (None, Some(p)) => shown(p),
        (None, None) => "git from your PATH".to_owned(),
    };
    st.git.info = Some(info);
    st.git.banner_dismissed = false;
    st.log(true, format!("Trellis now runs {what}"));
    // Another git may come with another git-lfs.
    super::lfs::handle(st, LfsMsg::Detect, now, fx);
}

fn refused(st: &mut AppState, program: &Path, why: String, now: Instant) {
    refused_text(st, shown(program), why, now);
}

/// Shows why a choice was refused on the page, in a toast and in the log; nothing else changes.
fn refused_text(st: &mut AppState, shown: String, why: String, now: Instant) {
    let text = format!("Can't use {shown}: {why}");
    st.toast(false, text.clone(), now);
    st.log(false, text);
    st.git.page.refusal = Some((shown, why));
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

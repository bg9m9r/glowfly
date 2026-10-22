//! The git Glowfly runs (the startup check, the banner and its offer of the PATH git when the
//! saved git can't run, features turned off for an old git, choosing another git in
//! Preferences > Git) and repositories git refuses because another user
//! owns them (explained, with an offer to list them in `safe.directory`).

use crate::effect::Effect;
use crate::git_env::{GitEnvEffect, GitEnvMsg, not_owned_text};
use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::state::git_env::Checking;
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::git_program::{Candidate, Refusal};
use glowfly_core::git_version::{Feature, GitInfo, show};
use glowfly_core::native_path::shown;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: GitEnvMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        GitEnvMsg::Detect => fx.push(Effect::GitEnv(GitEnvEffect::Detect { cwd: st.any_dir() })),
        GitEnvMsg::Detected(info) => {
            // Look again detects again: an unchanged answer isn't logged twice, and a changed
            // one shows the banner again even after a Dismiss.
            if st.git.info.as_ref() != Some(&info) {
                if let Some(text) = info.banner() {
                    st.log_quiet(false, text);
                }
                if st.git.info.is_some() {
                    st.git.banner_dismissed = false;
                }
            }
            look_for_path_git(&mut st.git.path_git, &info, fx);
            st.git.info = Some(info);
        }
        GitEnvMsg::PathGitChecked(c) => {
            if st.git.saved_git_unusable() {
                st.git.path_git = c;
            }
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
            let value = glowfly_core::safe_directory::config_value(&root, cfg!(windows));
            st.log(true, format!("Added {value} to safe.directory in your global git config"));
            super::repo::handle(st, RepoMsg::Open(root), now, fx);
        }
        GitEnvMsg::SafeDirectoryAdded { root, result: Err(e) } => {
            let text = format!("Could not add {} to safe.directory: {e}", glowfly_core::native_path::shown(&root));
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
        GitEnvMsg::Rescan => {
            discover(st, fx);
            // A saved git deleted while Glowfly runs is noticed here.
            if glowfly_core::git_program::from_setting(st.settings.git_program.as_deref()).is_some() {
                fx.push(Effect::GitEnv(GitEnvEffect::Detect { cwd: st.any_dir() }));
            }
        }
        GitEnvMsg::Discovered { generation, candidates } => {
            if generation == st.git.page.generation {
                st.git.page.candidates = Some(candidates);
                // The scan is fresher than the background check.
                if st.git.saved_git_unusable() {
                    st.git.path_git = st.git.page.path_git().cloned();
                }
            }
        }
        GitEnvMsg::Choose(program) => choose(st, program, false, fx),
        GitEnvMsg::UsePathGit => {
            if st.git.path_offer().is_some() {
                choose(st, None, true, fx);
            }
        }
        GitEnvMsg::Browse => {
            if st.git.page.checking.is_none() {
                fx.push(Effect::GitEnv(GitEnvEffect::PickProgram));
            }
        }
        GitEnvMsg::Picked(None) => {}
        GitEnvMsg::Picked(Some(p)) => {
            if p.to_str().is_some() {
                choose(st, Some(p), false, fx);
            } else {
                refused(st, &p, Refusal::NotUnicode.text(), now);
            }
        }
        GitEnvMsg::Switched { program, result } => {
            if st.git.page.checking.as_ref().is_none_or(|c| c.program != program) {
                return;
            }
            let from_banner = st.git.page.checking.take().is_some_and(|c| c.from_banner);
            match result {
                Ok(info) => switched(st, program, info, from_banner, now, fx),
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

/// Starts the check of the PATH git when `info` says the saved git can't run; any other answer
/// drops the last one.
fn look_for_path_git(path_git: &mut Option<Candidate>, info: &GitInfo, fx: &mut Vec<Effect>) {
    *path_git = None;
    if info.chosen && info.version.is_none() {
        fx.push(Effect::GitEnv(GitEnvEffect::CheckPathGit { cwd: AppState::outside_dir() }));
    }
}

/// Checks `program` (None: git from PATH) before it is used; one check at a time. A check
/// `from_banner` (its offer of the PATH git) is toasted when it succeeds.
fn choose(st: &mut AppState, program: Option<PathBuf>, from_banner: bool, fx: &mut Vec<Effect>) {
    let page = &mut st.git.page;
    if page.checking.is_some() {
        return;
    }
    page.refusal = None;
    page.checking = Some(Checking { program: program.clone(), from_banner });
    fx.push(Effect::GitEnv(GitEnvEffect::Use { program, cwd: AppState::outside_dir() }));
}

/// The runner has put `program` into effect: save it and follow the new git everywhere.
fn switched(st: &mut AppState, program: Option<PathBuf>, info: GitInfo, from_banner: bool, now: Instant, fx: &mut Vec<Effect>) {
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
    let toast = match (from_banner, info.version, info.path.as_deref()) {
        (true, Some(v), Some(p)) => {
            Some(st.settings.lang().trf("git_banner.switched.toast", &[("version", &show(v)), ("path", &shown(p))]))
        }
        _ => None,
    };
    st.git.info = Some(info);
    st.git.banner_dismissed = false;
    st.git.path_git = None;
    st.log(true, format!("Glowfly now runs {what}"));
    if let Some(text) = toast {
        st.toast(true, text, now);
    }
    // Another git may come with another git-lfs.
    super::lfs::handle(st, LfsMsg::Detect, now, fx);
    // From the banner with the page open: the list shows the PATH git as the one in use.
    if from_banner && st.prefs.as_ref().is_some_and(|p| p.page == PrefPage::Git) {
        discover(st, fx);
    }
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
    let shown = glowfly_core::native_path::shown(root);
    let value = glowfly_core::safe_directory::config_value(root, cfg!(windows));
    Modal::Confirm {
        title: "Repository owned by another user".into(),
        body: format!(
            "Git won't open {shown} because the folder belongs to another user account. This is usual for folders on a \
             network share or in WSL, on a FAT or exFAT drive, or copied from another computer. Git refuses them because a \
             repository's own settings can make git run programs.\n\nIf you trust this folder, Glowfly can list it in \
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

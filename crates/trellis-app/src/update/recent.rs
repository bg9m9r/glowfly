//! The recent and favorite repository lists: removing and clearing recent entries, toggling,
//! reordering and opening favorites, marking folders that are gone and removing them.

use crate::effect::Effect;
use crate::msg::{FolderCheck, Msg, RecentMsg};
use crate::state::repo_lists::{favorites, forget_confirm, is_favorite, is_listed, is_missing, paths_to_check};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::paths;

pub(super) fn handle(st: &mut AppState, msg: RecentMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RecentMsg::Remove(p) => remove_recent(st, &p, fx),
        RecentMsg::ClearRecent => {
            if !st.recent.is_empty() {
                st.recent.clear();
                fx.push(Effect::SaveRecent(Vec::new()));
            }
        }
        RecentMsg::SetFavorite { path, favorite: true } => {
            if !is_favorite(st, &path) {
                st.settings.favorite_repos.push(paths::encode(&path));
                fx.push(super::prefs::save_settings(st));
            }
        }
        RecentMsg::SetFavorite { path, favorite: false } => unfavorite(st, &path, fx),
        RecentMsg::CheckFolders => check_folders(st, fx),
        RecentMsg::FoldersChecked(checks) => folders_checked(st, checks),
        RecentMsg::MoveFavorite { from, to } => {
            let favs = &mut st.settings.favorite_repos;
            if from != to && from < favs.len() && to < favs.len() {
                let f = favs.remove(from);
                favs.insert(to, f);
                fx.push(super::prefs::save_settings(st));
            }
        }
        RecentMsg::OpenFavorite(n) => match favorites(&st.settings).into_iter().nth(n) {
            Some(p) => super::tabs::open(st, p, fx),
            None => st.toast(false, format!("There is no favorite repository {}. Star a repository to add it.", n + 1), now),
        },
        RecentMsg::OpenAll(paths) => super::tabs::open_all(st, paths, fx),
        RecentMsg::RemoveMissing => remove_missing(st, fx),
        RecentMsg::Forget(p) => {
            remove_recent(st, &p, fx);
            unfavorite(st, &p, fx);
        }
    }
}

fn remove_recent(st: &mut AppState, p: &Path, fx: &mut Vec<Effect>) {
    let before = st.recent.len();
    st.recent.retain(|r| !paths::same_spelling(r, p));
    if st.recent.len() != before {
        fx.push(Effect::SaveRecent(st.recent.clone()));
    }
}

fn unfavorite(st: &mut AppState, p: &Path, fx: &mut Vec<Effect>) {
    let favorites = std::mem::take(&mut st.settings.favorite_repos);
    let before = favorites.len();
    st.settings.favorite_repos = favorites.into_iter().filter(|f| !st.same_folder(&paths::decode(f), p)).collect();
    if st.settings.favorite_repos.len() != before {
        fx.push(super::prefs::save_settings(st));
    }
}

/// Drops every entry the last check found missing from both lists, saving each list that
/// changed.
fn remove_missing(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.missing_repos.is_empty() {
        return;
    }
    let recent = std::mem::take(&mut st.recent);
    let before = recent.len();
    st.recent = recent.into_iter().filter(|r| !is_missing(st, r)).collect();
    if st.recent.len() != before {
        fx.push(Effect::SaveRecent(st.recent.clone()));
    }
    let favorites = std::mem::take(&mut st.settings.favorite_repos);
    let before = favorites.len();
    st.settings.favorite_repos = favorites.into_iter().filter(|f| !is_missing(st, &paths::decode(f))).collect();
    if st.settings.favorite_repos.len() != before {
        fx.push(super::prefs::save_settings(st));
    }
    st.missing_repos.clear();
}

/// Asks which of the recent and favorite folders are gone (startup, window focus).
pub(super) fn check_folders(st: &AppState, fx: &mut Vec<Effect>) {
    let paths = paths_to_check(st);
    if !paths.is_empty() {
        fx.push(Effect::CheckRepoFolders(paths));
    }
}

/// A listed repository failed to open: checks the folders again, and once the check says
/// `path` is gone, offers to remove it from the lists (`folders_checked`).
pub(super) fn open_failed(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    if is_listed(st, &path) {
        st.missing_offer = Some(path);
        check_folders(st, fx);
    }
}

/// Marks the folders found missing, and remembers which spellings name one folder. A folder
/// that failed to open and is gone gets the question whether to remove it, unless another
/// dialog is open.
fn folders_checked(st: &mut AppState, checks: Vec<FolderCheck>) {
    st.missing_repos = checks.iter().filter(|c| !c.present).map(|c| c.path.clone()).collect();
    for c in checks {
        match c.folder {
            Some(id) => st.known.learn(&c.path, id),
            None => st.known.forget(&c.path),
        }
    }
    if let Some(p) = st.missing_offer.take().filter(|p| is_missing(st, p))
        && st.modal.is_none()
    {
        let (title, body) = forget_confirm(st, &p);
        let then = Box::new(Msg::Recent(RecentMsg::Forget(p)));
        st.modal = Some(Modal::Confirm { title, body, button: "Remove".into(), then });
    }
}

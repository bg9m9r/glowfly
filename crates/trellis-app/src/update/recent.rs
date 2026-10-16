//! The recent and favorite repository lists: removing and clearing recent entries, toggling
//! favorites, and marking folders that are gone.

use crate::effect::Effect;
use crate::msg::{FolderCheck, RecentMsg};
use crate::state::AppState;
use crate::state::repo_lists::{is_favorite, paths_to_check};
use trellis_core::paths;

pub(super) fn handle(st: &mut AppState, msg: RecentMsg, fx: &mut Vec<Effect>) {
    match msg {
        RecentMsg::Remove(p) => {
            let before = st.recent.len();
            st.recent.retain(|r| !paths::same_spelling(r, &p));
            if st.recent.len() != before {
                fx.push(Effect::SaveRecent(st.recent.clone()));
            }
        }
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
        RecentMsg::SetFavorite { path, favorite: false } => {
            let favorites = std::mem::take(&mut st.settings.favorite_repos);
            let before = favorites.len();
            st.settings.favorite_repos = favorites.into_iter().filter(|f| !st.same_folder(&paths::decode(f), &path)).collect();
            if st.settings.favorite_repos.len() != before {
                fx.push(super::prefs::save_settings(st));
            }
        }
        RecentMsg::CheckFolders => check_folders(st, fx),
        RecentMsg::FoldersChecked(checks) => folders_checked(st, checks),
    }
}

/// Asks which of the recent and favorite folders are gone (startup, window focus).
pub(super) fn check_folders(st: &AppState, fx: &mut Vec<Effect>) {
    let paths = paths_to_check(st);
    if !paths.is_empty() {
        fx.push(Effect::CheckRepoFolders(paths));
    }
}

/// Marks the folders found missing, and remembers which spellings name one folder.
fn folders_checked(st: &mut AppState, checks: Vec<FolderCheck>) {
    st.missing_repos = checks.iter().filter(|c| !c.present).map(|c| c.path.clone()).collect();
    for c in checks {
        match c.folder {
            Some(id) => st.known.learn(&c.path, id),
            None => st.known.forget(&c.path),
        }
    }
}

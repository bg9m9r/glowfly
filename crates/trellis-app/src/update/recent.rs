//! The recent and favorite repository lists: removing and clearing recent entries, toggling
//! favorites, and marking folders that are gone.

use crate::effect::Effect;
use crate::msg::RecentMsg;
use crate::state::AppState;
use crate::state::repo_lists::{is_favorite, paths_to_check};
use crate::state::tabs::same_path;
use std::path::Path;

pub(super) fn handle(st: &mut AppState, msg: RecentMsg, fx: &mut Vec<Effect>) {
    match msg {
        RecentMsg::Remove(p) => {
            let before = st.recent.len();
            st.recent.retain(|r| !same_path(r, &p));
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
            if !is_favorite(&st.settings, &path) {
                st.settings.favorite_repos.push(path.display().to_string());
                fx.push(super::prefs::save_settings(st));
            }
        }
        RecentMsg::SetFavorite { path, favorite: false } => {
            let before = st.settings.favorite_repos.len();
            st.settings.favorite_repos.retain(|f| !same_path(Path::new(f), &path));
            if st.settings.favorite_repos.len() != before {
                fx.push(super::prefs::save_settings(st));
            }
        }
        RecentMsg::CheckFolders => check_folders(st, fx),
        RecentMsg::FoldersChecked(missing) => st.missing_repos = missing,
    }
}

/// Asks which of the recent and favorite folders are gone (startup, window focus).
pub(super) fn check_folders(st: &AppState, fx: &mut Vec<Effect>) {
    let paths = paths_to_check(st);
    if !paths.is_empty() {
        fx.push(Effect::CheckRepoFolders(paths));
    }
}

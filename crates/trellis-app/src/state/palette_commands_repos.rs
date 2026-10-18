//! The palette's repository-list entries: favorites, then recent repositories, to open (each
//! but the active one and missing ones), and the list commands: favorite or unfavorite the open
//! repository, open all favorites in tabs, remove missing entries, find repositories in folders,
//! and fetch or pull the favorites or every listed repository.

use super::{Entry, PaletteCommand, entry, folder_name, with_detail};
use crate::keymap::{BindingId, FAVORITE_BINDINGS};
use crate::msg::{Msg, RecentMsg, RepoMsg, ReposMsg};
use crate::state::AppState;
use crate::state::repo_lists::{favorites, is_favorite, is_missing, missing_counts, remove_missing_msg, unopened_favorites};
use crate::state::repo_sync::{SyncKind, SyncScope, targets};
use std::path::{Path, PathBuf};

/// Recent repositories offered by the palette.
const MAX_RECENT: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum RepoListCommand {
    /// Open the favorite at `index` (its key hint is Open favorite repository `index + 1`).
    OpenFavorite {
        index: usize,
        path: PathBuf,
    },
    /// Add the open repository to favorites (`true`) or remove it.
    SetActiveFavorite(bool),
    OpenAllFavorites,
    /// Ask, then remove the missing entries from both lists.
    RemoveMissing,
    FindRepos,
    /// Fetch or pull the favorites or every listed repository.
    Sync {
        kind: SyncKind,
        scope: SyncScope,
    },
}

impl RepoListCommand {
    /// The binding that does the same thing (the first nine favorites').
    pub(crate) fn binding(&self) -> Option<BindingId> {
        match self {
            Self::OpenFavorite { index, .. } => FAVORITE_BINDINGS.get(*index).copied(),
            _ => None,
        }
    }

    /// The key it is remembered by among recent commands; None for a particular favorite.
    pub fn recall_key(&self) -> Option<&'static str> {
        Some(match self {
            Self::OpenFavorite { .. } => return None,
            Self::SetActiveFavorite(_) => "favorite-repo",
            Self::OpenAllFavorites => "open-all-favorites",
            Self::RemoveMissing => "remove-missing",
            Self::FindRepos => "find-repos",
            Self::Sync { kind: SyncKind::Fetch, scope: SyncScope::Favorites } => "fetch-favorites",
            Self::Sync { kind: SyncKind::Pull, scope: SyncScope::Favorites } => "pull-favorites",
            Self::Sync { kind: SyncKind::Fetch, scope: SyncScope::Listed } => "fetch-listed",
            Self::Sync { kind: SyncKind::Pull, scope: SyncScope::Listed } => "pull-listed",
        })
    }

    /// The message it runs; None when what it needs is gone (no open repository, nothing missing).
    pub fn msg(self, st: &AppState) -> Option<Msg> {
        Some(match self {
            Self::OpenFavorite { path, .. } => Msg::Repo(RepoMsg::Open(path)),
            Self::SetActiveFavorite(favorite) => Msg::Recent(RecentMsg::SetFavorite { path: st.snap.as_ref()?.workdir.clone(), favorite }),
            Self::OpenAllFavorites => Msg::Recent(RecentMsg::OpenAll(favorites(&st.settings))),
            Self::RemoveMissing => remove_missing_msg(st)?,
            Self::FindRepos => Msg::Repos(ReposMsg::ShowFinder),
            Self::Sync { kind, scope } => Msg::Repos(ReposMsg::StartSync { kind, scope }),
        })
    }
}

/// Whether `path` is the active tab's repository.
fn is_active(st: &AppState, path: &Path) -> bool {
    st.workdir().as_deref().is_some_and(|w| st.same_folder(w, path)) || st.snap.as_ref().is_some_and(|s| st.names_repo(s, path))
}

/// The favorites, then the recent repositories that aren't favorites, other than the active
/// one and missing ones.
pub(super) fn repo_entries(st: &AppState, out: &mut Vec<Entry>) {
    let skip = |p: &Path| is_active(st, p) || is_missing(st, p);
    for (index, path) in favorites(&st.settings).into_iter().enumerate() {
        if skip(&path) {
            continue;
        }
        let (label, detail) = (format!("Open {}", folder_name(&path)), trellis_core::native_path::shown(&path));
        let command = PaletteCommand::RepoList(RepoListCommand::OpenFavorite { index, path });
        out.push(with_detail(entry(command, label, "favorite starred repository"), Some(detail)));
    }
    for path in st.recent.iter().filter(|p| !is_favorite(st, p)).take(MAX_RECENT) {
        if skip(path) {
            continue;
        }
        let e = entry(PaletteCommand::OpenRecent(path.clone()), format!("Open {}", folder_name(path)), "recent repository");
        out.push(with_detail(e, Some(trellis_core::native_path::shown(path))));
    }
}

fn repositories(n: usize) -> String {
    if n == 1 { "1 repository".to_owned() } else { format!("{n} repositories") }
}

/// The favorite and recent list commands that apply now.
pub(super) fn list_entries(st: &AppState, out: &mut Vec<Entry>) {
    use RepoListCommand as R;
    let cmd = PaletteCommand::RepoList;
    if let Some(snap) = &st.snap {
        let fav = is_favorite(st, &snap.workdir);
        let label = if fav { "Remove this repository from favorites" } else { "Add this repository to favorites" };
        out.push(entry(cmd(R::SetActiveFavorite(!fav)), label, "favorite star unstar bookmark pin"));
    }
    let unopened = unopened_favorites(st).len();
    if unopened > 0 {
        let e = entry(cmd(R::OpenAllFavorites), "Open all favorites in tabs", "favorite starred repositories tabs every");
        out.push(with_detail(e, Some(repositories(unopened))));
    }
    let (recent, favs) = missing_counts(st);
    if recent + favs > 0 {
        let label = "Remove missing repositories from the lists";
        let e = entry(cmd(R::RemoveMissing), label, "missing gone deleted moved not found recent favorites clean");
        out.push(with_detail(e, Some((recent + favs).to_string())));
    }
    out.push(entry(cmd(R::FindRepos), "Find repositories in folders…", "scan search project folders discover directory repositories"));
    sync_entries(st, out);
}

/// Fetch or pull the favorites and every listed repository, when there are any, each with how
/// many repositories it takes in (missing ones left out).
fn sync_entries(st: &AppState, out: &mut Vec<Entry>) {
    let favs = !st.settings.favorite_repos.is_empty();
    if !favs && st.recent.is_empty() {
        return;
    }
    let all = [
        (SyncKind::Fetch, SyncScope::Favorites, "Fetch all favorites"),
        (SyncKind::Pull, SyncScope::Favorites, "Pull all favorites"),
        (SyncKind::Fetch, SyncScope::Listed, "Fetch all listed repositories"),
        (SyncKind::Pull, SyncScope::Listed, "Pull all listed repositories"),
    ];
    for (kind, scope, label) in all {
        if scope == SyncScope::Favorites && !favs {
            continue;
        }
        let n = targets(st, scope).len();
        let command = PaletteCommand::RepoList(RepoListCommand::Sync { kind, scope });
        let e = entry(command, label, "many every repositories update download remote sync bulk recent starred");
        out.push(with_detail(e, Some(repositories(n))));
    }
}

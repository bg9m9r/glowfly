//! The recent and favorite repository lists: which rows each section shows, which folders to
//! check, and the wording shared by the views and `update` (update/recent.rs).

use super::AppState;
use super::tabs::same_path;
use std::path::{Path, PathBuf};
use trellis_core::settings::Settings;

/// The favorite repositories, in the order they were added.
pub fn favorites(s: &Settings) -> Vec<PathBuf> {
    s.favorite_repos.iter().map(PathBuf::from).collect()
}

/// Whether `p` is a favorite, under any spelling of its path.
pub fn is_favorite(s: &Settings, p: &Path) -> bool {
    s.favorite_repos.iter().any(|f| same_path(Path::new(f), p))
}

/// Whether the last check found `p`'s folder missing.
pub fn is_missing(st: &AppState, p: &Path) -> bool {
    st.missing_repos.iter().any(|m| same_path(m, p))
}

/// The rows of the Recent section: the recent list without the favorites, which have their own.
pub fn recent_rows(st: &AppState) -> Vec<&PathBuf> {
    st.recent.iter().filter(|p| !is_favorite(&st.settings, p)).collect()
}

/// Every listed folder, favorites first, each once.
pub fn paths_to_check(st: &AppState) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in favorites(&st.settings).into_iter().chain(st.recent.iter().cloned()) {
        if !out.iter().any(|o| same_path(o, &p)) {
            out.push(p);
        }
    }
    out
}

/// The label of the favorite toggle for a row that is (`fav`) or isn't a favorite.
pub fn favorite_label(fav: bool) -> &'static str {
    if fav { "Remove from favorites" } else { "Add to favorites" }
}

/// The title and body of the question asked before clearing `n` recent entries.
pub fn clear_recent_confirm(n: usize) -> (String, String) {
    let what = if n == 1 { "1 repository".to_owned() } else { format!("{n} repositories") };
    let body = format!("Remove all {what} from the Recent list? Nothing on disk is touched, and favorites are kept.");
    ("Clear recent repositories".to_owned(), body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn state(recent: &[&str], favs: &[&str]) -> AppState {
        let settings = Settings { favorite_repos: favs.iter().map(|f| f.to_string()).collect(), ..Settings::default() };
        AppState::new(settings, recent.iter().map(PathBuf::from).collect(), Instant::now())
    }

    #[test]
    fn recent_rows_leave_out_favorites() {
        let st = state(&["/w/a", "/w/b", "/w/c"], &["/w/b/"]);
        assert_eq!(recent_rows(&st), [&PathBuf::from("/w/a"), &PathBuf::from("/w/c")]);
        if cfg!(any(windows, target_os = "macos")) {
            let st = state(&["C:/Work/A", "C:/work/b"], &["c:\\work\\a"]);
            assert_eq!(recent_rows(&st), [&PathBuf::from("C:/work/b")]);
        }
    }

    #[test]
    fn paths_to_check_put_favorites_first_once() {
        let st = state(&["/w/a", "/w/b"], &["/w/c", "/w/a/"]);
        let want: Vec<PathBuf> = ["/w/c", "/w/a/", "/w/b"].iter().map(PathBuf::from).collect();
        assert_eq!(paths_to_check(&st), want);
    }

    #[test]
    fn missing_compares_spelling_only() {
        let mut st = state(&["/w/a"], &[]);
        st.missing_repos = vec![PathBuf::from("/w/a/")];
        assert!(is_missing(&st, Path::new("/w/./a")));
        assert!(!is_missing(&st, Path::new("/w/b")));
    }

    #[test]
    fn clear_confirmation_counts_repositories() {
        assert!(clear_recent_confirm(1).1.contains("Remove all 1 repository from"));
        assert!(clear_recent_confirm(3).1.contains("Remove all 3 repositories from"));
        assert_eq!(clear_recent_confirm(3).0, "Clear recent repositories");
        assert_eq!(favorite_label(true), "Remove from favorites");
        assert_eq!(favorite_label(false), "Add to favorites");
    }
}

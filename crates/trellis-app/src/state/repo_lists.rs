//! The recent and favorite repository lists: which rows each section shows, which folders to
//! check, which are open in a tab or missing, and the wording shared by the views and `update`
//! (update/recent.rs).

use super::AppState;
use crate::keymap::{FAVORITE_BINDINGS, Keymap};
use std::path::{Path, PathBuf};
use trellis_core::settings::Settings;

/// The tip of the "open" tag on a listed repository that a tab shows.
pub const OPEN_TIP: &str = "Open in a tab. Click to switch to it.";

/// The favorite repositories, in the order the user put them.
pub fn favorites(s: &Settings) -> Vec<PathBuf> {
    s.favorite_repos.iter().map(|f| trellis_core::paths::decode(f)).collect()
}

/// Whether `p` is a favorite: the same folder as one (`AppState::same_folder`).
pub fn is_favorite(st: &AppState, p: &Path) -> bool {
    st.settings.favorite_repos.iter().any(|f| st.same_folder(&trellis_core::paths::decode(f), p))
}

/// Whether the last check found `p`'s folder missing.
pub fn is_missing(st: &AppState, p: &Path) -> bool {
    st.missing_repos.iter().any(|m| trellis_core::paths::same_spelling(m, p))
}

/// The rows of the Recent section: the recent list without the favorites, which have their own.
pub fn recent_rows(st: &AppState) -> Vec<&PathBuf> {
    st.recent.iter().filter(|p| !is_favorite(st, p)).collect()
}

/// Every listed folder, favorites first, each spelling once (each is looked at, so the check
/// finds which are one folder).
pub fn paths_to_check(st: &AppState) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in favorites(&st.settings).into_iter().chain(st.recent.iter().cloned()) {
        if !out.iter().any(|o| trellis_core::paths::same_spelling(o, &p)) {
            out.push(p);
        }
    }
    out
}

/// Whether a tab (the active one included) shows the folder at `p`.
pub fn is_open(st: &AppState, p: &Path) -> bool {
    st.tab_showing(p).is_some()
}

/// Whether `p` is a favorite or on the Recent list.
pub fn is_listed(st: &AppState, p: &Path) -> bool {
    is_favorite(st, p) || st.recent.iter().any(|r| st.same_folder(r, p))
}

/// The favorites Open all would add a tab for: not shown by a tab and not missing.
pub fn unopened_favorites(st: &AppState) -> Vec<PathBuf> {
    favorites(&st.settings).into_iter().filter(|p| !is_open(st, p) && !is_missing(st, p)).collect()
}

/// How many rows the last check found missing: (Recent section, Favorites section).
pub fn missing_counts(st: &AppState) -> (usize, usize) {
    let recent = recent_rows(st).into_iter().filter(|p| is_missing(st, p)).count();
    let favs = favorites(&st.settings).iter().filter(|p| is_missing(st, p)).count();
    (recent, favs)
}

/// `n` repositories, in words.
fn repositories(n: usize) -> String {
    if n == 1 { "1 repository".to_owned() } else { format!("{n} repositories") }
}

/// The title and body of the question asked before removing the missing entries.
pub fn remove_missing_confirm(recent: usize, favs: usize) -> (String, String) {
    let total = repositories(recent + favs);
    let lists = match (recent, favs) {
        (0, _) => "the Favorites list".to_owned(),
        (_, 0) => "the Recent list".to_owned(),
        (r, f) => format!("the Favorites and Recent lists ({f} in Favorites, {r} in Recent)"),
    };
    let body = format!("Remove {total} that can't be found from {lists}? Nothing on disk is touched.");
    ("Remove missing repositories".to_owned(), body)
}

/// The question asked before the missing entries are removed (welcome screen, repository menu,
/// palette); None when the last check found nothing missing.
pub fn remove_missing_msg(st: &AppState) -> Option<crate::msg::Msg> {
    use crate::msg::{Msg, RecentMsg};
    let (recent, favs) = missing_counts(st);
    if recent + favs == 0 {
        return None;
    }
    let (title, body) = remove_missing_confirm(recent, favs);
    Some(Msg::confirm(&title, body, "Remove", Msg::Recent(RecentMsg::RemoveMissing)))
}

/// The title and body of the question asked when a listed repository that failed to open is
/// gone.
pub fn forget_confirm(st: &AppState, p: &Path) -> (String, String) {
    let lists = match (is_favorite(st, p), st.recent.iter().any(|r| st.same_folder(r, p))) {
        (true, true) => "your Favorites and Recent lists",
        (true, false) => "your Favorites",
        _ => "your Recent list",
    };
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| trellis_core::native_path::shown(p));
    let body = format!(
        "{name} can't be found at {}. It may have been moved or deleted, or be on a drive that isn't connected. Remove it from {lists}?",
        trellis_core::native_path::shown(p)
    );
    ("Repository not found".to_owned(), body)
}

/// The keys part of the note under the Favorites header ("The first nine open with … to …."),
/// read from `keys` so a rebound key shows; None when the first favorite has no key.
pub fn favorite_keys_note(keys: &Keymap) -> Option<String> {
    let first = keys.hint(FAVORITE_BINDINGS[0])?;
    Some(match keys.hint(FAVORITE_BINDINGS[8]) {
        Some(last) => format!("The first nine open with {first} to {last}."),
        None => format!("The first opens with {first}."),
    })
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
        // Case is the volume's business: another case is another row until a check finds both
        // spellings to be one folder.
        let mut st = state(&["/Work/A", "/work/b"], &["/work/a"]);
        assert_eq!(recent_rows(&st), [&PathBuf::from("/Work/A"), &PathBuf::from("/work/b")]);
        st.known.learn(Path::new("/Work/A"), trellis_core::paths::FolderId::for_tests(1));
        st.known.learn(Path::new("/work/a"), trellis_core::paths::FolderId::for_tests(1));
        assert_eq!(recent_rows(&st), [&PathBuf::from("/work/b")]);
        // Only Windows reads a backslash as a separator.
        if cfg!(windows) {
            let st = state(&["C:/work/A", "C:/work/b"], &[r"c:\work\A"]);
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
    fn open_and_missing_rows() {
        let mut st = state(&["/w/a", "/w/b", "/w/c"], &["/w/c", "/w/d", "/w/e"]);
        st.missing_repos = vec![PathBuf::from("/w/b"), PathBuf::from("/w/e")];
        st.tabs = vec![crate::state::Tab::unloaded(PathBuf::from("/w/d/"))];
        assert!(is_open(&st, Path::new("/w/d")) && !is_open(&st, Path::new("/w/c")));
        assert_eq!(unopened_favorites(&st), [PathBuf::from("/w/c")], "open and missing ones are left out");
        assert_eq!(missing_counts(&st), (1, 1));
        assert!(is_listed(&st, Path::new("/w/a/")) && is_listed(&st, Path::new("/w/e")) && !is_listed(&st, Path::new("/w/z")));
    }

    #[test]
    fn missing_questions_name_the_lists() {
        let (title, body) = remove_missing_confirm(2, 1);
        assert_eq!(title, "Remove missing repositories");
        assert_eq!(
            body,
            "Remove 3 repositories that can't be found from the Favorites and Recent lists (1 in Favorites, 2 in Recent)? Nothing on disk is touched."
        );
        assert!(remove_missing_confirm(1, 0).1.starts_with("Remove 1 repository that can't be found from the Recent list?"));
        assert!(remove_missing_confirm(0, 2).1.contains("from the Favorites list?"));
        let st = state(&["/w/a", "/w/b"], &["/w/b"]);
        let (title, body) = forget_confirm(&st, Path::new("/w/a"));
        assert_eq!(title, "Repository not found");
        assert!(body.starts_with("a can't be found at "), "{body}");
        assert!(body.ends_with("Remove it from your Recent list?"), "{body}");
        assert!(forget_confirm(&st, Path::new("/w/b")).1.ends_with("your Favorites and Recent lists?"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn keys_note_follows_the_keymap() {
        let keys = Keymap::default();
        assert_eq!(favorite_keys_note(&keys).as_deref(), Some("The first nine open with Ctrl+Alt+1 to Ctrl+Alt+9."));
        let mut keys = keys;
        keys.set(FAVORITE_BINDINGS[8], Vec::new());
        assert_eq!(favorite_keys_note(&keys).as_deref(), Some("The first opens with Ctrl+Alt+1."));
        keys.set(FAVORITE_BINDINGS[0], Vec::new());
        assert_eq!(favorite_keys_note(&keys), None);
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

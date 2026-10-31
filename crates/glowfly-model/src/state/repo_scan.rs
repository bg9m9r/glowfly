//! The Find repositories dialog: the project folders it looks in, the last scan's results and
//! the rows it draws. Global (`AppState::repo_scan`), never parked with a tab; changed by
//! update/repo_scan.rs, scanned by runner/repos.rs.

use super::AppState;
use super::repo_lists::{is_favorite, is_open};
use glowfly_core::paths::FolderId;
use glowfly_core::settings::Settings;
use std::path::PathBuf;

/// The title of the folder picker that adds a project folder.
pub const PICK_TITLE: &str = "Add a project folder";

/// The last scan of the project folders, and whether one is running.
#[derive(Default)]
pub struct RepoScan {
    /// Incremented for every scan started; results of older scans are ignored.
    pub generation: u64,
    pub running: bool,
    /// The repositories found, sorted by folder name, each once.
    pub found: Vec<FoundRepo>,
    /// Project folders that couldn't be listed, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
    /// The scan stopped at `repo_scan::MAX_FOLDERS`.
    pub truncated: bool,
}

/// A repository the scan found.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundRepo {
    /// The project folder it was found under.
    pub root: PathBuf,
    pub path: PathBuf,
    /// Its folder's name.
    pub name: String,
    /// Its identity, when the volume gives one.
    pub folder: Option<FolderId>,
}

/// One result row as the dialog draws it.
pub struct FinderRow {
    pub repo: FoundRepo,
    /// A tab shows it.
    pub open: bool,
    pub favorite: bool,
}

/// The project folders, in the order they were added.
pub fn project_folders(s: &Settings) -> Vec<PathBuf> {
    s.project_folders.iter().map(|f| glowfly_core::paths::decode(f)).collect()
}

/// Whether `row` matches the filter: every whitespace-separated term appears in its name or
/// its shown path, ignoring case and Unicode form (for matching only).
pub fn matches(row: &FoundRepo, filter: &str) -> bool {
    let hay = glowfly_core::names::fold(&format!("{}\n{}", row.name, glowfly_core::native_path::shown(&row.path)));
    filter.split_whitespace().all(|t| hay.contains(&glowfly_core::names::fold(t)))
}

/// The rows of the last scan, with whether each is open in a tab or a favorite.
pub fn finder_rows(st: &AppState) -> Vec<FinderRow> {
    st.repo_scan
        .found
        .iter()
        .map(|r| FinderRow { repo: r.clone(), open: is_open(st, &r.path), favorite: is_favorite(st, &r.path) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn repo(path: &str) -> FoundRepo {
        let path = PathBuf::from(path);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        FoundRepo { root: PathBuf::from("/src"), path, name, folder: None }
    }

    #[test]
    fn every_term_has_to_match_name_or_path() {
        let r = repo("/src/clients/Acme-API");
        assert!(matches(&r, ""));
        assert!(matches(&r, "api"));
        assert!(matches(&r, "  ACME   clients "), "terms in any order and case");
        assert!(!matches(&r, "api web"));
        // Composed and decomposed forms match each other.
        let r = repo("/src/caf\u{e9}");
        assert!(matches(&r, "cafe\u{301}"));
    }

    #[test]
    fn rows_know_open_tabs_and_favorites() {
        let settings = Settings { favorite_repos: vec!["/src/b".into()], ..Settings::default() };
        let mut st = AppState::new(settings, Vec::new(), Instant::now());
        st.tabs = vec![crate::state::Tab::unloaded(PathBuf::from("/src/a"))];
        st.repo_scan.found = vec![repo("/src/a"), repo("/src/b")];
        let rows = finder_rows(&st);
        assert!(rows[0].open && !rows[0].favorite);
        assert!(!rows[1].open && rows[1].favorite);
    }
}

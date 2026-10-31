//! The Find repositories dialog: opening it, adding and removing project folders (saved in
//! settings), and scanning them in the runner. The results are global (`st.repo_scan`).

use crate::effect::OsEffect;
use crate::effect::{Effect, ReposEffect};
use crate::msg::{FolderPurpose, ScanFound};
use crate::state::repo_scan::{PICK_TITLE, project_folders};
use crate::state::{AppState, Modal};
use glowfly_core::paths;
use std::path::{Path, PathBuf};

/// Opens the dialog with an empty filter and scans the project folders again; the last
/// results stay on screen until the new ones arrive.
pub(super) fn show(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.modal = Some(Modal::FindRepos { filter: String::new() });
    rescan(st, fx);
}

pub(super) fn add_folder(fx: &mut Vec<Effect>) {
    fx.push(Effect::Os(OsEffect::PickFolder { title: PICK_TITLE.into(), purpose: FolderPurpose::ProjectFolder }));
}

/// A project folder was picked: adds it unless it is listed already, then scans.
pub(super) fn folder_picked(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    if project_folders(&st.settings).iter().any(|f| st.same_folder(f, &path)) {
        return;
    }
    st.settings.project_folders.push(paths::encode(&path));
    fx.push(super::prefs::save_settings(st));
    rescan(st, fx);
}

/// Stops looking in `root`: drops it and what was found under it, then scans the rest.
pub(super) fn remove_folder(st: &mut AppState, root: &Path, fx: &mut Vec<Effect>) {
    let before = st.settings.project_folders.len();
    st.settings.project_folders.retain(|f| !paths::same_spelling(&paths::decode(f), root));
    if st.settings.project_folders.len() == before {
        return;
    }
    fx.push(super::prefs::save_settings(st));
    st.repo_scan.found.retain(|r| !paths::same_spelling(&r.root, root));
    st.repo_scan.unreadable.retain(|(r, _)| !paths::same_spelling(r, root));
    rescan(st, fx);
}

/// Scans the project folders; with none, clears the results.
pub(super) fn rescan(st: &mut AppState, fx: &mut Vec<Effect>) {
    let roots = project_folders(&st.settings);
    st.repo_scan.generation += 1;
    if roots.is_empty() {
        let generation = st.repo_scan.generation;
        st.repo_scan = crate::state::repo_scan::RepoScan { generation, ..Default::default() };
        return;
    }
    st.repo_scan.running = true;
    fx.push(Effect::Repos(ReposEffect::Scan { generation: st.repo_scan.generation, roots }));
}

/// A scan finished: shows its results (unless a newer scan started) and remembers each
/// repository's identity, so the "open" tags match a tab that opened it under another spelling.
pub(super) fn scanned(st: &mut AppState, generation: u64, report: ScanFound) {
    if generation != st.repo_scan.generation {
        return;
    }
    for r in &report.found {
        if let Some(id) = r.folder {
            st.known.learn(&r.path, id);
        }
    }
    let scan = &mut st.repo_scan;
    scan.running = false;
    scan.found = report.found;
    scan.unreadable = report.unreadable;
    scan.truncated = report.truncated;
}

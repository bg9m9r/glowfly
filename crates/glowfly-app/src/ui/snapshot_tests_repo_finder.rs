//! The Find repositories dialog: project folders, the filter, results grouped by folder with
//! star and "open" tags, and its empty, scanning and truncated states. The paths are labels
//! only, never read.

use super::snapshot_tests::{harness, settings};
use crate::msg::{Msg, RepoMsg, ReposMsg};
use crate::state::repo_scan::FoundRepo;
use crate::state::{AppState, Modal, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

/// A path that is absolute on every system (`C:/…` on Windows, `/…` elsewhere).
fn abs(rel: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { format!("C:/{rel}") } else { format!("/{rel}") })
}

fn found(root: &str, rel: &str) -> FoundRepo {
    let path = abs(&format!("{root}/{rel}"));
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    FoundRepo { root: abs(root), path, name, folder: None }
}

/// Two project folders with five repositories, api-server open in a tab and web-app a favorite.
pub(super) fn finder(theme: ThemeChoice, filter: &str) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.settings.project_folders = [abs("src"), abs("work")].iter().map(|p| glowfly_core::paths::encode(p)).collect();
    st.settings.favorite_repos = vec![glowfly_core::paths::encode(&abs("src/web-app"))];
    st.repo_scan.found = vec![
        found("src", "api-server"),
        found("src", "clients/billing"),
        found("work", "docs"),
        found("work", "tools/scripts"),
        found("src", "web-app"),
    ];
    st.tabs = vec![Tab::empty(), Tab::unloaded(abs("src/api-server"))];
    st.modal = Some(Modal::FindRepos { filter: filter.into() });
    st
}

fn shown(p: &std::path::Path) -> String {
    glowfly_core::native_path::shown(p)
}

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn msgs(h: &H) -> &[Msg] {
    &h.state().1
}

#[test]
fn repo_finder() {
    let mut h = harness(finder(ThemeChoice::Dark, ""), true);
    h.run();
    for label in
        ["Find repositories", "Project folders", "Add folder…", "Look again", "api-server", "billing", "docs", "scripts", "web-app"]
    {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert_eq!(h.query_all_by_label("open").count(), 1, "api-server is in a tab");
    // The welcome screen under the dialog lists web-app too, starred.
    assert_eq!(h.query_all_by_label("Add to favorites").count(), 4, "web-app is a favorite");
    assert!(h.query_by_label(&format!("{} (3)", shown(&abs("src")))).is_some(), "a header per folder with its count");
    h.check_snapshot("repo_finder");
    h.get_by_label("docs").click();
    h.run();
    assert!(matches!(msgs(&h), [Msg::CloseModal, Msg::Repo(RepoMsg::Open(p))] if *p == abs("work/docs")), "opens and closes");
}

#[test]
fn repo_finder_folder_buttons() {
    let mut h = harness(finder(ThemeChoice::Dark, ""), true);
    h.run();
    let cross = h.get_all_by_label("Stop looking in this folder").nth(1).expect("a × per folder");
    cross.click();
    h.run();
    assert!(matches!(msgs(&h), [Msg::Repos(ReposMsg::RemoveFolder(p))] if *p == abs("work")));
    h.state_mut().1.clear();
    h.get_by_label("Look again").click();
    h.run();
    h.get_by_label("Add folder…").click();
    h.run();
    assert!(matches!(msgs(&h), [Msg::Repos(ReposMsg::Rescan), Msg::Repos(ReposMsg::AddFolder)]));
}

#[test]
fn repo_finder_filtered_light() {
    let mut h = harness(finder(ThemeChoice::Light, "api"), false);
    h.run();
    assert!(h.query_by_label("api-server").is_some());
    assert!(h.query_by_label("docs").is_none() && h.query_by_label("web-app").is_none());
    assert!(h.query_by_label(&format!("{} (1)", shown(&abs("src")))).is_some());
    h.check_snapshot("repo_finder_filtered_light");
    let mut h = harness(finder(ThemeChoice::Dark, "nothing-like-this"), true);
    h.run();
    assert!(h.query_by_label("No repositories match").is_some());
}

#[test]
fn repo_finder_empty() {
    let mut st = finder(ThemeChoice::Dark, "");
    st.settings.project_folders.clear();
    st.repo_scan.found.clear();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Add a folder you keep repositories in, and Trellis lists the Git repositories under it.").is_some());
    assert!(h.query_by_label("Look again").is_none());
    h.check_snapshot("repo_finder_empty");
}

#[test]
fn repo_finder_scanning() {
    let mut st = finder(ThemeChoice::Dark, "");
    st.repo_scan.running = true;
    st.repo_scan.truncated = true;
    st.repo_scan.unreadable = vec![(abs("work"), "Access is denied.".into())];
    let mut h = harness(st, true);
    h.step();
    for label in ["Looking for repositories…", "can't be read: Access is denied."] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert!(h.query_by_label("Stopped after looking in 20000 folders. Add a folder closer to your repositories.").is_some());
    h.check_snapshot("repo_finder_scanning");
    // Nothing found yet while the first scan runs: no "nothing found" note.
    let mut st = finder(ThemeChoice::Dark, "");
    st.repo_scan.found.clear();
    st.repo_scan.running = true;
    let mut h = harness(st, true);
    h.step();
    assert!(h.query_by_label("No Git repositories were found in these folders.").is_none());
}

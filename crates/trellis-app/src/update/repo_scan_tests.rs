//! The Find repositories dialog: project folders, scanning them in the runner, stale results.

use super::tests::{drive, git, state};
use super::update;
use crate::effect::{Effect, ReposEffect};
use crate::msg::{FolderPurpose, Msg, PrefsMsg, ReposMsg, ScanFound};
use crate::state::repo_scan::{FoundRepo, project_folders};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, m: ReposMsg) -> Vec<Effect> {
    update(st, Msg::Repos(m), Instant::now())
}

/// A folder that is absolute on every system (nothing is read from it).
fn abs(rel: &str) -> PathBuf {
    if cfg!(windows) { PathBuf::from(format!("C:/{rel}")) } else { PathBuf::from(format!("/{rel}")) }
}

fn scans(fx: &[Effect]) -> Vec<(u64, Vec<PathBuf>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Repos(ReposEffect::Scan { generation, roots }) => Some((*generation, roots.clone())),
            _ => None,
        })
        .collect()
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

#[test]
fn show_without_folders_opens_the_dialog_and_scans_nothing() {
    let mut st = state();
    let fx = run(&mut st, ReposMsg::ShowFinder);
    assert!(matches!(&st.modal, Some(Modal::FindRepos { filter }) if filter.is_empty()));
    assert!(scans(&fx).is_empty(), "{fx:?}");
    assert!(!st.repo_scan.running);
}

#[test]
fn show_with_folders_starts_a_new_scan() {
    let mut st = state();
    st.settings.project_folders = vec![trellis_core::paths::encode(&abs("src"))];
    let before = st.repo_scan.generation;
    let fx = run(&mut st, ReposMsg::ShowFinder);
    assert_eq!(scans(&fx), [(before + 1, vec![abs("src")])]);
    assert!(st.repo_scan.running);
    // Look again starts another, and only its results count.
    let again = scans(&run(&mut st, ReposMsg::Rescan));
    assert_eq!(again[0].0, before + 2);
}

#[test]
fn stale_results_are_ignored() {
    let mut st = state();
    st.settings.project_folders = vec![trellis_core::paths::encode(&abs("src"))];
    run(&mut st, ReposMsg::ShowFinder);
    let old = st.repo_scan.generation;
    run(&mut st, ReposMsg::Rescan);
    let repo = FoundRepo { root: abs("src"), path: abs("src/a"), name: "a".into(), folder: None };
    let report = Box::new(ScanFound { found: vec![repo.clone()], ..Default::default() });
    run(&mut st, ReposMsg::Scanned { generation: old, report });
    assert!(st.repo_scan.found.is_empty() && st.repo_scan.running, "an older scan's results");
    let report = Box::new(ScanFound { found: vec![repo.clone()], truncated: true, ..Default::default() });
    let generation = st.repo_scan.generation;
    run(&mut st, ReposMsg::Scanned { generation, report });
    assert_eq!(st.repo_scan.found, [repo]);
    assert!(st.repo_scan.truncated && !st.repo_scan.running);
}

#[test]
fn a_listed_folder_is_not_added_twice() {
    let mut st = state();
    let picked = |st: &mut AppState, p: PathBuf| {
        update(st, Msg::Prefs(PrefsMsg::FolderPicked { purpose: FolderPurpose::ProjectFolder, path: Some(p) }), Instant::now())
    };
    let fx = picked(&mut st, abs("src"));
    assert_eq!(project_folders(&st.settings), [abs("src")]);
    assert_eq!(saves(&fx), 1);
    assert_eq!(scans(&fx).len(), 1, "adding a folder scans");
    let mut other = abs("src").into_os_string();
    other.push("/");
    let fx = picked(&mut st, PathBuf::from(other));
    assert_eq!(project_folders(&st.settings), [abs("src")], "the same folder with a trailing slash");
    assert!(fx.is_empty(), "{fx:?}");
}

#[test]
fn removing_a_folder_saves_and_scans_the_rest() {
    let mut st = state();
    st.settings.project_folders = [abs("src"), abs("work")].iter().map(|p| trellis_core::paths::encode(p)).collect();
    st.repo_scan.found = vec![FoundRepo { root: abs("src"), path: abs("src/a"), name: "a".into(), folder: None }];
    let fx = run(&mut st, ReposMsg::RemoveFolder(abs("src")));
    assert_eq!(project_folders(&st.settings), [abs("work")]);
    assert!(st.repo_scan.found.is_empty(), "what was found under it goes");
    assert_eq!(saves(&fx), 1);
    assert_eq!(scans(&fx).iter().map(|(_, r)| r.clone()).collect::<Vec<_>>(), [vec![abs("work")]]);
    // The last folder gone: nothing to scan, and the results are cleared.
    let fx = run(&mut st, ReposMsg::RemoveFolder(abs("work")));
    assert!(scans(&fx).is_empty() && saves(&fx) == 1);
    assert!(!st.repo_scan.running);
    assert!(run(&mut st, ReposMsg::RemoveFolder(abs("elsewhere"))).is_empty(), "not a project folder");
}

#[test]
fn add_folder_asks_for_a_folder() {
    let mut st = state();
    let fx = run(&mut st, ReposMsg::AddFolder);
    assert!(matches!(&fx[..], [Effect::PickFolder { purpose: FolderPurpose::ProjectFolder, .. }]), "{fx:?}");
}

/// A project folder holding two repositories, one level down and two levels down.
fn project(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-app-scan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (a, b) = (dir.join("api"), dir.join("clients").join("web"));
    for r in [&a, &b] {
        std::fs::create_dir_all(r).unwrap();
        git(r, &["init", "-q"]);
    }
    (dir, a, b)
}

#[test]
fn scanning_end_to_end_lists_repositories_with_their_identity() {
    let (dir, a, b) = project("e2e");
    let mut st = state();
    st.settings.project_folders = vec![trellis_core::paths::encode(&dir)];
    drive(&mut st, Msg::Repos(ReposMsg::ShowFinder));
    let found: Vec<&Path> = st.repo_scan.found.iter().map(|r| r.path.as_path()).collect();
    assert_eq!(found, [a.as_path(), b.as_path()]);
    assert_eq!(st.repo_scan.found[1].name, "web");
    assert!(st.repo_scan.found.iter().all(|r| r.root == dir));
    assert!(!st.repo_scan.running);
    if let Some(id) = st.repo_scan.found[0].folder {
        assert_eq!(st.known.id_of(&a), Some(id), "the identity is learned for the open tags");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

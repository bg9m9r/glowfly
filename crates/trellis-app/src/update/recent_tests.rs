//! The recent and favorite repository lists: removing, clearing, favoriting and missing folders.

use super::tests::{drive, same_place, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_lists::is_missing;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, m: RecentMsg) -> Vec<Effect> {
    update(st, Msg::Recent(m), Instant::now())
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn favorite(st: &mut AppState, p: &str, favorite: bool) -> Vec<Effect> {
    run(st, RecentMsg::SetFavorite { path: PathBuf::from(p), favorite })
}

/// The favorites carried by the `SaveSettings` effects in `fx`, one list per effect.
fn saved_favorites(fx: &[Effect]) -> Vec<Vec<String>> {
    fx.iter().filter_map(|e| if let Effect::SaveSettings(s) = e { Some(s.favorite_repos.clone()) } else { None }).collect()
}

#[test]
fn remove_drops_the_entry_and_saves() {
    let mut st = state();
    st.recent = paths(&["/w/a", "/w/b", "/w/c"]);
    let fx = run(&mut st, RecentMsg::Remove(PathBuf::from("/w/b/")));
    assert_eq!(st.recent, paths(&["/w/a", "/w/c"]));
    assert!(matches!(&fx[..], [Effect::SaveRecent(l)] if *l == paths(&["/w/a", "/w/c"])), "{fx:?}");
}

#[test]
fn remove_of_unknown_path_does_nothing() {
    let mut st = state();
    st.recent = paths(&["/w/a"]);
    let fx = run(&mut st, RecentMsg::Remove(PathBuf::from("/w/z")));
    assert_eq!(st.recent, paths(&["/w/a"]));
    assert!(fx.is_empty(), "{fx:?}");
}

#[test]
fn remove_keeps_the_favorite() {
    let mut st = state();
    st.recent = paths(&["/w/a", "/w/b"]);
    st.settings.favorite_repos = vec!["/w/a".into()];
    let fx = run(&mut st, RecentMsg::Remove(PathBuf::from("/w/a")));
    assert_eq!(st.recent, paths(&["/w/b"]));
    assert_eq!(st.settings.favorite_repos, ["/w/a"]);
    assert!(saved_favorites(&fx).is_empty(), "settings are not saved: {fx:?}");
}

#[test]
fn clear_recent_empties_the_list_and_keeps_favorites() {
    let mut st = state();
    st.recent = paths(&["/w/a", "/w/b"]);
    st.settings.favorite_repos = vec!["/w/b".into()];
    let fx = run(&mut st, RecentMsg::ClearRecent);
    assert!(st.recent.is_empty());
    assert_eq!(st.settings.favorite_repos, ["/w/b"]);
    assert!(matches!(&fx[..], [Effect::SaveRecent(l)] if l.is_empty()), "{fx:?}");
    assert!(run(&mut st, RecentMsg::ClearRecent).is_empty(), "nothing to clear");
}

#[test]
fn favorite_is_added_once_in_order_and_saved() {
    let mut st = state();
    let fx = favorite(&mut st, "/w/a", true);
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    assert_eq!(saved_favorites(&fx), [vec!["/w/a".to_owned()]]);
    let fx = favorite(&mut st, "/w/b", true);
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    assert_eq!(saved_favorites(&fx), [vec!["/w/a".to_owned(), "/w/b".to_owned()]]);
    assert_eq!(st.settings.favorite_repos, ["/w/a", "/w/b"]);
    let fx = favorite(&mut st, "/w/./a/", true);
    assert!(fx.is_empty(), "already a favorite under another spelling: {fx:?}");
    assert_eq!(st.settings.favorite_repos, ["/w/a", "/w/b"]);
}

#[test]
fn unfavorite_removes_and_saves() {
    let mut st = state();
    st.settings.favorite_repos = vec!["/w/a".into(), "/w/b".into()];
    let fx = favorite(&mut st, "/w/a/", false);
    assert_eq!(st.settings.favorite_repos, ["/w/b"]);
    assert_eq!(saved_favorites(&fx), [vec!["/w/b".to_owned()]]);
    assert_eq!(fx.len(), 1, "{fx:?}");
    assert!(favorite(&mut st, "/w/z", false).is_empty(), "not a favorite");
}

#[test]
fn check_folders_covers_favorites_and_recent() {
    let mut st = state();
    assert!(run(&mut st, RecentMsg::CheckFolders).is_empty(), "nothing listed");
    st.recent = paths(&["/w/a", "/w/b"]);
    st.settings.favorite_repos = vec!["/w/c".into(), "/w/b".into()];
    let fx = run(&mut st, RecentMsg::CheckFolders);
    assert!(matches!(&fx[..], [Effect::CheckRepoFolders(l)] if *l == paths(&["/w/c", "/w/b", "/w/a"])), "{fx:?}");
}

#[test]
fn window_focus_checks_folders() {
    let dir = scratch_repo("recent-focus");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    let fx = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::CheckRepoFolders(l) if l.len() == 1)), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "the refresh still runs: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_folders_are_found_end_to_end() {
    let dir = scratch_repo("recent-missing");
    let missing = dir.join("gone");
    let mut st = state();
    st.recent = vec![dir.clone(), missing.clone()];
    drive(&mut st, Msg::Recent(RecentMsg::CheckFolders));
    assert_eq!(st.missing_repos, [missing]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_repository_clears_its_missing_mark() {
    let dir = scratch_repo("recent-reopen");
    let mut st = state();
    st.missing_repos = vec![dir.clone()];
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(!is_missing(&st, &dir), "{:?}", st.missing_repos);
    assert!(same_place(&st.recent[0], &dir), "{:?}", st.recent);
    let _ = std::fs::remove_dir_all(&dir);
}

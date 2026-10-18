//! The recent and favorite repository lists: removing, clearing, favoriting and missing folders.

use super::tests::{drive, same_place, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RecentMsg, RepoMsg};
use crate::state::repo_lists::is_missing;
use crate::state::{AppState, Modal};
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

#[test]
fn move_favorite_reorders_and_saves_once() {
    let mut st = state();
    st.settings.favorite_repos = vec!["/w/a".into(), "/w/b".into(), "/w/c".into()];
    let fx = run(&mut st, RecentMsg::MoveFavorite { from: 2, to: 0 });
    assert_eq!(st.settings.favorite_repos, ["/w/c", "/w/a", "/w/b"]);
    assert_eq!(saved_favorites(&fx), [vec!["/w/c".to_owned(), "/w/a".into(), "/w/b".into()]]);
    let fx = run(&mut st, RecentMsg::MoveFavorite { from: 0, to: 1 });
    assert_eq!(st.settings.favorite_repos, ["/w/a", "/w/c", "/w/b"]);
    assert_eq!(fx.len(), 1, "{fx:?}");
    assert!(run(&mut st, RecentMsg::MoveFavorite { from: 1, to: 1 }).is_empty(), "no move");
    assert!(run(&mut st, RecentMsg::MoveFavorite { from: 3, to: 0 }).is_empty(), "out of range");
    assert!(run(&mut st, RecentMsg::MoveFavorite { from: 0, to: 3 }).is_empty(), "out of range");
    assert_eq!(st.settings.favorite_repos, ["/w/a", "/w/c", "/w/b"]);
}

#[test]
fn open_favorite_opens_it_or_says_there_is_none() {
    let dir = scratch_repo("recent-open-favorite");
    let mut st = state();
    st.settings.favorite_repos = vec![trellis_core::paths::encode(&dir), "/w/other".into()];
    drive(&mut st, Msg::Recent(RecentMsg::OpenFavorite(0)));
    assert!(st.snap.as_ref().is_some_and(|s| same_place(&s.workdir, &dir)), "the first favorite is open");
    let fx = run(&mut st, RecentMsg::OpenFavorite(5));
    assert!(fx.is_empty(), "{fx:?}");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "There is no favorite repository 6. Star a repository to add it.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_all_adds_tabs_for_unopened_present_favorites() {
    let mut st = state();
    st.settings.favorite_repos = vec!["/w/a".into(), "/w/b".into(), "/w/c".into()];
    st.tabs = vec![crate::state::Tab::unloaded(PathBuf::from("/w/a"))];
    st.missing_repos = vec![PathBuf::from("/w/b")];
    let fx = run(&mut st, RecentMsg::OpenAll(paths(&["/w/a", "/w/b", "/w/c"])));
    let tabs: Vec<Option<PathBuf>> = st.tabs.iter().map(|t| t.path.clone()).collect();
    assert_eq!(tabs, [Some(PathBuf::from("/w/a")), Some(PathBuf::from("/w/c"))], "one new tab: /w/a is open, /w/b missing");
    assert_eq!(st.active_tab, 1);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveTabs { .. })), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, .. } if path == &PathBuf::from("/w/c"))), "{fx:?}");
}

#[test]
fn open_all_with_everything_open_switches_to_the_first() {
    let mut st = state();
    st.settings.favorite_repos = vec!["/w/a".into()];
    st.tabs = vec![crate::state::Tab::empty(), crate::state::Tab::unloaded(PathBuf::from("/w/a"))];
    run(&mut st, RecentMsg::OpenAll(paths(&["/w/a"])));
    assert_eq!(st.tabs.len(), 2);
    assert_eq!(st.active_tab, 1);
}

#[test]
fn remove_missing_cleans_both_lists() {
    let mut st = state();
    assert!(run(&mut st, RecentMsg::RemoveMissing).is_empty(), "nothing missing");
    st.recent = paths(&["/w/a", "/w/b", "/w/c"]);
    st.settings.favorite_repos = vec!["/w/c/".into(), "/w/d".into()];
    st.missing_repos = paths(&["/w/b", "/w/c"]);
    let fx = run(&mut st, RecentMsg::RemoveMissing);
    assert_eq!(st.recent, paths(&["/w/a"]));
    assert_eq!(st.settings.favorite_repos, ["/w/d"]);
    assert!(st.missing_repos.is_empty());
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveRecent(l) if *l == paths(&["/w/a"]))), "{fx:?}");
    assert_eq!(saved_favorites(&fx), [vec!["/w/d".to_owned()]]);
    // Only the list that changed is saved.
    st.recent = paths(&["/w/e"]);
    st.missing_repos = paths(&["/w/e"]);
    let fx = run(&mut st, RecentMsg::RemoveMissing);
    assert!(matches!(&fx[..], [Effect::SaveRecent(l)] if l.is_empty()), "{fx:?}");
}

#[test]
fn forget_drops_a_folder_from_both_lists() {
    let mut st = state();
    st.recent = paths(&["/w/a", "/w/b"]);
    st.settings.favorite_repos = vec!["/w/a".into()];
    let fx = run(&mut st, RecentMsg::Forget(PathBuf::from("/w/./a/")));
    assert_eq!(st.recent, paths(&["/w/b"]));
    assert!(st.settings.favorite_repos.is_empty());
    assert_eq!(fx.len(), 2, "both lists saved: {fx:?}");
}

/// A folder under the temporary folder that doesn't exist.
fn gone(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join(format!("trellis-app-test-{name}-{}-gone", std::process::id()))
}

#[test]
fn a_listed_folder_that_is_gone_offers_to_forget_it() {
    let path = gone("forget-offer");
    let mut st = state();
    st.recent = vec![path.clone()];
    drive(&mut st, Msg::Repo(RepoMsg::Open(path.clone())));
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("asks to remove it") };
    assert_eq!(title, "Repository not found");
    assert!(body.ends_with("Remove it from your Recent list?"), "{body}");
    assert_eq!(button, "Remove");
    assert!(matches!(&**then, Msg::Recent(RecentMsg::Forget(p)) if *p == path));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(st.recent.is_empty());
}

#[test]
fn other_failed_opens_ask_nothing() {
    // Not listed.
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(gone("forget-unlisted"))));
    assert!(st.modal.is_none());
    // Listed, and there, but not a repository: some other load error.
    let plain = gone("forget-plain");
    std::fs::create_dir_all(&plain).unwrap();
    let mut st = state();
    st.recent = vec![plain.clone()];
    drive(&mut st, Msg::Repo(RepoMsg::Open(plain.clone())));
    assert!(st.modal.is_none());
    assert!(st.missing_offer.is_none());
    let _ = std::fs::remove_dir_all(&plain);
}

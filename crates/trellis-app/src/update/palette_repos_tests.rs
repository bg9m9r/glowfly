//! The palette's favorite and recent repositories and the list commands: order, what is left
//! out, key hints, conditions and recall keys.

use super::palette_tests::repo_with_topic;
use super::tests::state;
use super::update;
use crate::keymap::BindingId;
use crate::msg::{Msg, PaletteMsg, RecentMsg, ReposMsg};
use crate::state::palette::{PaletteCommand as C, shown};
use crate::state::palette_commands::RepoListCommand as R;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;

fn labels(st: &AppState) -> Vec<String> {
    shown(st, "").into_iter().map(|s| s.entry.label).collect()
}

fn has(st: &AppState, label: &str) -> bool {
    labels(st).iter().any(|l| l == label)
}

fn run_cmd(st: &mut AppState, cmd: C) {
    update(st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now());
}

#[test]
fn favorites_come_before_recent_repositories() {
    let mut st = state();
    st.recent = ["/w/recent-a", "/w/fav-b", "/w/gone-c"].iter().map(PathBuf::from).collect();
    st.settings.favorite_repos = vec!["/w/fav-a".into(), "/w/fav-b".into(), "/w/gone-f".into()];
    st.missing_repos = vec![PathBuf::from("/w/gone-c"), PathBuf::from("/w/gone-f")];
    let opens: Vec<String> = labels(&st).into_iter().filter(|l| l.starts_with("Open ") && l.contains('-')).collect();
    assert_eq!(opens, ["Open fav-a", "Open fav-b", "Open recent-a"], "a favorite once, missing ones left out");
    let list = shown(&st, "");
    let first = list.iter().find(|s| s.entry.label == "Open fav-a").unwrap();
    assert!(matches!(&first.entry.command, C::RepoList(R::OpenFavorite { index: 0, .. })));
    assert_eq!(first.entry.shortcut(&st.keymap), st.keymap.hint(BindingId::OpenFavorite1));
    assert!(first.entry.shortcut(&st.keymap).is_some());
    let second = list.iter().find(|s| s.entry.label == "Open fav-b").unwrap();
    assert_eq!(second.entry.shortcut(&st.keymap), st.keymap.hint(BindingId::OpenFavorite2));
}

#[test]
fn the_active_repository_is_left_out_and_can_be_starred() {
    let (mut st, dir) = repo_with_topic("palette-favorite-active");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    st.settings.favorite_repos = vec![trellis_core::paths::encode(&workdir)];
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(!has(&st, &format!("Open {name}")), "the open repository");
    assert!(has(&st, "Remove this repository from favorites"));
    st.settings.favorite_repos.clear();
    assert!(has(&st, "Add this repository to favorites"));
    run_cmd(&mut st, C::RepoList(R::SetActiveFavorite(true)));
    assert_eq!(st.settings.favorite_repos.len(), 1);
    assert!(has(&st, "Remove this repository from favorites"), "the label flips");
    assert_eq!(st.palette_recent.keys(), ["favorite-repo"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_commands_show_when_they_apply() {
    let mut st = state();
    assert!(has(&st, "Find repositories in folders…"), "always");
    assert!(!has(&st, "Open all favorites in tabs") && !has(&st, "Remove missing repositories from the lists"));
    assert!(!has(&st, "Add this repository to favorites"), "no repository open");
    st.settings.favorite_repos = vec!["/w/a".into(), "/w/b".into()];
    st.recent = vec![PathBuf::from("/w/c")];
    st.missing_repos = vec![PathBuf::from("/w/b"), PathBuf::from("/w/c")];
    let list = shown(&st, "");
    let open_all = list.iter().find(|s| s.entry.label == "Open all favorites in tabs").expect("one favorite to open");
    assert_eq!(open_all.entry.detail.as_deref(), Some("1 repository"));
    let remove = list.iter().find(|s| s.entry.label == "Remove missing repositories from the lists").expect("two missing");
    assert_eq!(remove.entry.detail.as_deref(), Some("2"));
    run_cmd(&mut st, C::RepoList(R::RemoveMissing));
    let Some(Modal::Confirm { title, then, .. }) = &st.modal else { panic!("asks first") };
    assert_eq!(title, "Remove missing repositories");
    assert!(matches!(&**then, Msg::Recent(RecentMsg::RemoveMissing)));
    assert_eq!(st.palette_recent.keys(), ["remove-missing"]);
    st.modal = None;
    run_cmd(&mut st, C::RepoList(R::FindRepos));
    assert!(matches!(st.modal, Some(Modal::FindRepos { .. })));
    assert_eq!(st.palette_recent.keys(), ["find-repos", "remove-missing"]);
    assert_eq!(R::OpenAllFavorites.recall_key(), Some("open-all-favorites"));
    assert!(matches!(R::FindRepos.msg(&st), Some(Msg::Repos(ReposMsg::ShowFinder))));
}

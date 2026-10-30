//! Repository groups: making, renaming, colouring, ordering and removing groups, moving
//! favorites into, within and out of them, opening a group, and what un-starring, removing
//! missing entries and the tab colours do with groups.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::repo_colours::{listed_colour, tab_colour};
use crate::state::repo_groups::{members, ungrouped};
use crate::state::repo_lists::favorites;
use crate::state::{AppState, Modal, Tab};
use glowfly_core::settings::RepoColour;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, m: GroupMsg) -> Vec<Effect> {
    update(st, Msg::Groups(m), Instant::now())
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

fn move_to(st: &mut AppState, p: &str, group: Option<usize>, index: usize) -> Vec<Effect> {
    run(st, GroupMsg::MoveRepo { path: PathBuf::from(p), group, index })
}

/// Favorites a to d, and the groups Work (with c) and Home (empty), made through the messages.
fn grouped() -> AppState {
    let mut st = state();
    st.settings.favorite_repos = ["/w/a", "/w/b", "/w/c", "/w/d"].iter().map(|s| s.to_string()).collect();
    run(&mut st, GroupMsg::Create { name: "Work".into(), repo: Some(PathBuf::from("/w/c/")) });
    run(&mut st, GroupMsg::Create { name: "Home".into(), repo: None });
    st
}

#[test]
fn creating_groups_hands_out_colours_and_moves_the_repository_in() {
    let mut st = grouped();
    let names: Vec<&str> = st.settings.repo_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Work", "Home"]);
    let hues: Vec<_> = st.settings.repo_groups.iter().map(|g| g.hue()).collect();
    assert_eq!(hues, [Some(RepoColour::Rose), Some(RepoColour::Teal)], "far-apart hues first");
    assert_eq!(st.settings.repo_groups[0].repos, ["/w/c"], "the favorite's own spelling is kept");
    assert_eq!(ungrouped(&st.settings), paths(&["/w/a", "/w/b", "/w/d"]));
    assert!(run(&mut st, GroupMsg::Create { name: "  \n ".into(), repo: None }).is_empty(), "a blank name makes nothing");
    let fx = run(&mut st, GroupMsg::Create { name: "New".into(), repo: Some(PathBuf::from("/w/x")) });
    assert_eq!(saves(&fx), 1, "{fx:?}");
    assert!(st.settings.favorite_repos.contains(&"/w/x".to_owned()), "a repository moved into a group is starred");
    assert_eq!(members(&st.settings, 2), paths(&["/w/x"]));
}

#[test]
fn create_and_rename_refuse_a_name_in_use_or_blank() {
    let mut st = grouped();
    assert!(run(&mut st, GroupMsg::Create { name: " work ".into(), repo: Some(PathBuf::from("/w/a")) }).is_empty(), "Work exists");
    assert_eq!(st.settings.repo_groups.len(), 2);
    assert!(!st.settings.repo_groups.iter().any(|g| g.repos.contains(&"/w/a".to_owned())), "nothing moved either");
    assert!(run(&mut st, GroupMsg::Rename { group: 1, name: "WORK".into() }).is_empty(), "another group's name");
    assert!(run(&mut st, GroupMsg::Rename { group: 1, name: " \t ".into() }).is_empty(), "blank");
    assert_eq!(st.settings.repo_groups[1].name, "Home");
    assert_eq!(saves(&run(&mut st, GroupMsg::Rename { group: 0, name: "work".into() })), 1, "its own name in another case");
    assert_eq!(st.settings.repo_groups[0].name, "work");
}

#[test]
fn the_dialogs_open_with_the_right_form() {
    let mut st = grouped();
    run(&mut st, GroupMsg::NewDialog { repo: Some(PathBuf::from("/w/a")) });
    assert!(matches!(&st.modal, Some(Modal::RepoGroup(f)) if f.group.is_none() && f.text.is_empty() && f.repo.is_some()));
    run(&mut st, GroupMsg::RenameDialog(1));
    assert!(matches!(&st.modal, Some(Modal::RepoGroup(f)) if f.group == Some(1) && f.text == "Home" && f.taken == ["Work"]));
    st.modal = None;
    run(&mut st, GroupMsg::RenameDialog(9));
    assert!(st.modal.is_none(), "no such group");
}

#[test]
fn rename_recolour_and_reorder_save_only_changes() {
    let mut st = grouped();
    assert_eq!(saves(&run(&mut st, GroupMsg::Rename { group: 1, name: " Personal ".into() })), 1);
    assert_eq!(st.settings.repo_groups[1].name, "Personal");
    assert!(run(&mut st, GroupMsg::Rename { group: 1, name: "Personal".into() }).is_empty(), "same name");
    assert!(run(&mut st, GroupMsg::Rename { group: 1, name: " ".into() }).is_empty(), "blank");
    assert!(run(&mut st, GroupMsg::Rename { group: 7, name: "x".into() }).is_empty(), "gone");
    assert_eq!(saves(&run(&mut st, GroupMsg::SetColour { group: 0, colour: Some(RepoColour::Iris) })), 1);
    assert_eq!(st.settings.repo_groups[0].hue(), Some(RepoColour::Iris));
    assert!(run(&mut st, GroupMsg::SetColour { group: 0, colour: Some(RepoColour::Iris) }).is_empty());
    assert_eq!(saves(&run(&mut st, GroupMsg::SetColour { group: 0, colour: None })), 1);
    assert_eq!(saves(&run(&mut st, GroupMsg::MoveGroup { from: 1, to: 0 })), 1);
    let names: Vec<&str> = st.settings.repo_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Personal", "Work"]);
    assert!(run(&mut st, GroupMsg::MoveGroup { from: 0, to: 0 }).is_empty());
    assert!(run(&mut st, GroupMsg::MoveGroup { from: 0, to: 2 }).is_empty(), "out of range");
}

#[test]
fn moving_favorites_fills_reorders_and_empties_groups() {
    let mut st = grouped();
    assert_eq!(saves(&move_to(&mut st, "/w/a", Some(0), 0)), 1, "into Work, above c");
    assert_eq!(members(&st.settings, 0), paths(&["/w/a", "/w/c"]));
    move_to(&mut st, "/w/a", Some(0), usize::MAX);
    assert_eq!(members(&st.settings, 0), paths(&["/w/c", "/w/a"]), "reordered inside its group");
    move_to(&mut st, "/w/c", Some(1), 0);
    assert_eq!((members(&st.settings, 0), members(&st.settings, 1)), (paths(&["/w/a"]), paths(&["/w/c"])), "from one group to another");
    move_to(&mut st, "/w/c", None, 0);
    assert_eq!(ungrouped(&st.settings), paths(&["/w/c", "/w/b", "/w/d"]), "out of the groups, at the top");
    move_to(&mut st, "/w/c", None, usize::MAX);
    assert_eq!(ungrouped(&st.settings), paths(&["/w/b", "/w/d", "/w/c"]));
    move_to(&mut st, "/w/c", None, 1);
    assert_eq!(ungrouped(&st.settings), paths(&["/w/b", "/w/c", "/w/d"]), "Move up and Move down among the ungrouped");
    assert!(move_to(&mut st, "/w/c", None, 1).is_empty(), "already there");
    assert!(move_to(&mut st, "/w/c", Some(5), 0).is_empty(), "no such group");
    assert_eq!(favorites(&st.settings), paths(&["/w/a", "/w/b", "/w/c", "/w/d"]), "groups first, then the rest");
}

#[test]
fn the_favorite_keys_and_open_all_follow_the_groups() {
    let mut st = grouped();
    move_to(&mut st, "/w/d", Some(0), 0);
    let fx = update(&mut st, Msg::Recent(RecentMsg::OpenFavorite(0)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, .. } if path == &PathBuf::from("/w/d"))), "{fx:?}");
    let work = members(&st.settings, 0);
    let fx = update(&mut st, Msg::Recent(RecentMsg::OpenAll(work)), Instant::now());
    let tabs: Vec<Option<PathBuf>> = st.tabs.iter().map(|t| t.path.clone()).collect();
    assert!(tabs.contains(&Some(PathBuf::from("/w/c"))) && tabs.contains(&Some(PathBuf::from("/w/d"))), "{tabs:?}");
    assert!(!tabs.contains(&Some(PathBuf::from("/w/a"))), "only the group's");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveTabs { .. })), "{fx:?}");
}

#[test]
fn removing_a_group_keeps_its_repositories_as_favorites() {
    let mut st = grouped();
    move_to(&mut st, "/w/a", Some(0), usize::MAX);
    let fx = run(&mut st, GroupMsg::Remove(0));
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.repo_groups.len(), 1);
    assert_eq!(ungrouped(&st.settings), paths(&["/w/b", "/w/d", "/w/c", "/w/a"]), "after the ungrouped ones, in the group's order");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Removed the group Work. Its repositories are still in your favorites.");
    assert!(run(&mut st, GroupMsg::Remove(3)).is_empty(), "no such group");
}

#[test]
fn unstarring_and_removing_missing_take_repositories_out_of_groups() {
    let mut st = grouped();
    move_to(&mut st, "/w/a", Some(1), 0);
    let fx = update(&mut st, Msg::Recent(RecentMsg::SetFavorite { path: PathBuf::from("/w/c"), favorite: false }), Instant::now());
    assert_eq!(saves(&fx), 1);
    assert!(st.settings.repo_groups[0].repos.is_empty(), "un-starred, so out of its group");
    st.missing_repos = vec![PathBuf::from("/w/a")];
    update(&mut st, Msg::Recent(RecentMsg::RemoveMissing), Instant::now());
    assert!(st.settings.repo_groups[1].repos.is_empty());
    assert_eq!(st.settings.repo_groups.len(), 2, "the groups stay, empty");
}

#[test]
fn a_repository_without_a_colour_takes_its_groups() {
    let mut st = grouped();
    st.tabs = vec![Tab::unloaded(PathBuf::from("/w/c")), Tab::unloaded(PathBuf::from("/w/a"))];
    assert_eq!(listed_colour(&st, std::path::Path::new("/w/c")), Some(RepoColour::Rose));
    assert_eq!(tab_colour(&st, 0), Some(RepoColour::Rose), "the tab strip shows the group's colour");
    assert_eq!(tab_colour(&st, 1), None, "not in a group");
    st.settings.set_repo_colour(std::path::Path::new("/w/c"), Some(RepoColour::Sky));
    assert_eq!(tab_colour(&st, 0), Some(RepoColour::Sky), "its own colour wins");
    st.settings.set_repo_colour(std::path::Path::new("/w/c"), None);
    run(&mut st, GroupMsg::SetColour { group: 0, colour: None });
    assert_eq!(tab_colour(&st, 0), None);
}

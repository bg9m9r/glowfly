//! The toolbar's repository menu and its rows (`repo_menu_list.rs`, `repo_menu_row.rs`): the
//! lists first, one row per repository with its name, parent, status, star and ×, then the
//! commands and the tidying group; the star and × on hover or keyboard focus, Delete on a row,
//! the filter field of a long list, the arrows and typeahead between the rows, and the pixel
//! goldens in dark and light. The listed paths
//! are labels only, never opened.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_repo_list::{H, abs, api, last, lists, old, shown, web};
use super::snapshot_tests_select::click;
use crate::msg::{Msg, RecentMsg, RepoMsg, ReposMsg};
use crate::state::repo_sync::{SyncKind, SyncScope};
use crate::state::{AppState, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{RepoColour, ThemeChoice};
use std::path::{Path, PathBuf};

/// Opens the toolbar's repository menu: the button named by the repository, in the toolbar.
pub(super) fn open_repo_menu(h: &mut H) {
    let name = h
        .get_all_by_label("demo")
        .filter_map(|n| n.bounding_box())
        .filter(|b| b.y1 < 120.0)
        .min_by(|a, b| a.x0.total_cmp(&b.x0))
        .expect("the repository button");
    click(h, pos2(((name.x0 + name.x1) / 2.0) as f32, ((name.y0 + name.y1) / 2.0) as f32), PointerButton::Primary);
}

fn deep() -> String {
    abs("clients/northwind-traders/platform-team/services/billing-gateway")
}

/// The golden's lists: web-app (a favorite, with a colour), then Recent: the current
/// repository, api-server (in another tab), a long path, and old-site (missing).
fn golden_state(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let dark = theme == ThemeChoice::Dark;
    let (mut st, dir) = with_repo(theme, name);
    lists(&mut st);
    let current = abs("glowfly");
    st.snap.as_mut().unwrap().workdir = PathBuf::from(&current);
    st.recent = [current, api(), deep(), old()].iter().map(PathBuf::from).collect();
    st.tabs[1] = Tab::unloaded(PathBuf::from(api()));
    // Dark shows the swatches' slot; light the rows without one.
    if dark {
        st.settings.set_repo_colour(Path::new(&web()), Some(RepoColour::Teal));
        st.settings.set_repo_colour(Path::new(&api()), Some(RepoColour::Iris));
    }
    (st, dir)
}

fn golden(theme: ThemeChoice, dark: bool, name: &str) {
    let (st, dir) = golden_state(theme, name);
    let mut h = harness(st, dark);
    h.run();
    open_repo_menu(&mut h);
    h.get_by_label(&shown(&api())).hover();
    h.run();
    for label in ["Add to favorites", "Remove from recent"] {
        assert_eq!(h.query_all_by_label(label).count(), 1, "the hovered row shows {label}");
    }
    assert!(h.query_by_label("Add this repository to favorites").is_none(), "the current repository has a row");
    h.check_snapshot(name);
    remove_fixture(&dir);
}

#[test]
fn repo_menu_dark() {
    golden(ThemeChoice::Dark, true, "repo_menu_dark");
}

#[test]
fn repo_menu_light() {
    golden(ThemeChoice::Light, false, "repo_menu_light");
}

#[test]
fn repo_menu_rows_say_their_status() {
    let (st, dir) = golden_state(ThemeChoice::Dark, "repo-menu-status");
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    let status = |h: &H, path: &str| h.get_by_label(&shown(path)).description();
    assert_eq!(status(&h, &abs("glowfly")).as_deref(), Some("Current"));
    assert_eq!(status(&h, &api()).as_deref(), Some("In a tab"));
    assert_eq!(status(&h, &old()).as_deref(), Some("Not found"));
    assert_eq!(status(&h, &deep()), None);
    // Only the favorite's filled star shows while no row is hovered or focused.
    assert_eq!(h.query_all_by_label("Remove from favorites").count(), 1);
    assert!(h.query_by_label("Add to favorites").is_none() && h.query_by_label("Remove from recent").is_none());
    remove_fixture(&dir);
}

#[test]
fn repo_menu_favorites_and_recent() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu");
    lists(&mut st);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    for label in ["Add this repository to favorites", "Favorites", "Recent", "Clear recent…", &shown(&web()), &shown(&api())] {
        assert!(h.query_by_label(label).is_some(), "{label} is in the menu");
    }
    assert_eq!(h.query_all_by_label(&shown(&web())).count(), 1, "a favorite is listed once");
    // The lists come before the commands.
    let top = |h: &H, label: &str| h.get_by_label(label).bounding_box().expect(label).y0;
    assert!(top(&h, &shown(&old())) < top(&h, "Open repository…"));
    assert!(top(&h, "Open repository…") < top(&h, "Clear recent…"));
    h.check_snapshot("repo_menu");
    h.get_by_label("Add this repository to favorites").click();
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if *path == workdir));
    // The star and × act without closing the menu; the row opens and closes it.
    open_repo_menu(&mut h);
    h.get_by_label(&shown(&api())).hover();
    h.run();
    h.get_by_label("Add to favorites").click();
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if path == Path::new(&api())));
    h.get_by_label("Remove from recent").click();
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == Path::new(&api())));
    assert!(h.query_by_label("Open repository…").is_some(), "still open");
    h.get_by_label(&shown(&api())).click();
    h.run();
    assert!(matches!(last(&h), Msg::Repo(RepoMsg::Open(p)) if p == Path::new(&api())));
    assert!(h.query_by_label("Open repository…").is_none(), "opening closes the menu");
    remove_fixture(&dir);
}

#[test]
fn repo_menu_rows_work_from_the_keyboard() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu-keys");
    lists(&mut st);
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    h.get_by_label(&shown(&api())).focus();
    h.run();
    assert!(h.query_by_label("Add to favorites").is_some(), "a focused row shows its star");
    // Tab moves to the star, which stays shown while it has focus.
    h.press_key(Key::Tab);
    h.run();
    assert!(h.get_by_label("Add to favorites").is_focused());
    h.press_key(Key::Enter);
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if path == Path::new(&api())));
    h.press_key(Key::Tab);
    h.run();
    assert!(h.get_by_label("Remove from recent").is_focused(), "then to ×");
    // Delete on a focused Recent row removes it.
    h.get_by_label(&shown(&old())).focus();
    h.run();
    h.press_key(Key::Delete);
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == Path::new(&old())));
    h.press_key(Key::Enter);
    h.run();
    assert!(matches!(last(&h), Msg::Repo(RepoMsg::Open(p)) if p == Path::new(&old())), "Enter opens it");
    remove_fixture(&dir);
}

#[test]
fn repo_menu_filters_a_long_list() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu-filter");
    st.recent = (0..9).map(|i| PathBuf::from(abs(&format!("service-{i}")))).chain([PathBuf::from(abs("docs"))]).collect();
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    let field = h.get_by_label("Find a repository");
    assert!(field.is_focused(), "the filter takes focus when the menu opens");
    field.type_text("doc");
    h.run();
    assert!(h.query_by_label(&shown(&abs("docs"))).is_some());
    assert!(h.query_by_label(&shown(&abs("service-1"))).is_none(), "the others are hidden");
    h.check_snapshot("repo_menu_filter");
    h.press_key(Key::Enter);
    h.run();
    assert!(matches!(last(&h), Msg::Repo(RepoMsg::Open(p)) if p == Path::new(&abs("docs"))), "Enter opens the first match");
    // Opened again, the filter starts empty.
    open_repo_menu(&mut h);
    assert!(h.query_by_label(&shown(&abs("service-1"))).is_some());
    remove_fixture(&dir);
}

#[test]
fn repo_menu_actions() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu-actions");
    lists(&mut st);
    // api-server is in the second tab.
    st.tabs[1] = Tab::unloaded(PathBuf::from(api()));
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    for label in ["Find repositories in folders…", "Open all favorites in tabs", "Fetch or pull repositories", "Remove missing entry (1)…"]
    {
        assert!(h.query_by_label(label).is_some(), "{label} is in the menu");
    }
    h.get_by_label("Remove missing entry (1)…").click();
    h.run();
    assert!(matches!(last(&h), Msg::Confirm { then, .. } if matches!(**then, Msg::Recent(RecentMsg::RemoveMissing))));
    open_repo_menu(&mut h);
    h.get_by_label("Find repositories in folders…").click();
    h.run();
    assert!(matches!(last(&h), Msg::Repos(ReposMsg::ShowFinder)));
    // Fetch or pull, in a submenu.
    open_repo_menu(&mut h);
    h.get_by_label("Fetch or pull repositories").hover();
    h.run();
    for label in ["Fetch favorites", "Pull favorites", "Fetch all listed repositories", "Pull all listed repositories"] {
        assert!(h.query_by_label(label).is_some(), "{label} is in the submenu");
    }
    h.check_snapshot("repo_menu_sync");
    h.get_by_label("Pull favorites").click();
    h.run();
    assert!(matches!(last(&h), Msg::Repos(ReposMsg::StartSync { kind: SyncKind::Pull, scope: SyncScope::Favorites })));
    remove_fixture(&dir);
}

#[test]
fn repo_menu_rows_take_the_arrows_and_typeahead() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu-arrows");
    lists(&mut st);
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    let row = |p: String| shown(&p);
    h.get_by_label(&row(api())).focus();
    h.run();
    h.press_key(Key::ArrowDown);
    h.run();
    assert!(h.get_by_label(&row(old())).is_focused(), "Down goes to the next row, past the star and ×");
    h.press_key(Key::ArrowUp);
    h.run();
    assert!(h.get_by_label(&row(api())).is_focused(), "Up back");
    h.press_key(Key::ArrowRight);
    h.run();
    assert!(h.get_by_label("Add to favorites").is_focused(), "Right reaches the row's star");
    h.press_key(Key::ArrowLeft);
    h.run();
    assert!(h.get_by_label(&row(api())).is_focused(), "Left returns to the row");
    h.input_mut().events.push(eframe::egui::Event::Text("o".into()));
    h.run();
    assert!(h.get_by_label(&row(old())).is_focused(), "typeahead by name");
    h.press_key(Key::Tab);
    h.run();
    assert!(h.get_by_label("Add to favorites").is_focused(), "Tab still reaches the star");
    remove_fixture(&dir);
}

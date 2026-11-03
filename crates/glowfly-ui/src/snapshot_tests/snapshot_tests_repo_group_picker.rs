//! Adding repositories to a group without dragging: the Add repositories dialog (its two
//! sections, a favorite from another group, the count on Add, the filter on a long list, the
//! nothing-to-add state, German), the link in an empty group's placeholder, the group menu's
//! item and a Recent row's Add to group submenu. The listed paths are labels only, never opened.

use super::menu_testing::open_submenu;
use super::snapshot_tests::{harness, harness_sized};
use super::snapshot_tests_repo_list::{H, abs, api, last, shown, web, welcome_state};
use super::snapshot_tests_select::click;
use crate::msg::{GroupMsg, Msg};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{RepoColour, RepoGroup, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

fn centre(h: &H, label: &str) -> Pos2 {
    let b = h.get_all_by_label(label).next().and_then(|n| n.bounding_box()).expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// Work (Rose: web-app) and Home (Teal: api-server); docs and notes ungrouped; Recent cli and
/// site besides the favorites and the missing old-site.
fn picker_state(theme: ThemeChoice) -> AppState {
    let mut st = welcome_state(theme);
    st.settings.favorite_repos = vec![web(), api(), abs("docs"), abs("notes")];
    let mut work = RepoGroup::new("Work", Some(RepoColour::Rose));
    work.repos = vec![web()];
    let mut home = RepoGroup::new("Home", Some(RepoColour::Teal));
    home.repos = vec![api()];
    st.settings.repo_groups = vec![work, home];
    st.recent.extend([abs("cli"), abs("site")].map(PathBuf::from));
    st
}

/// Opens Work's picker through `update`, with the rows named in `tick` ticked.
fn open_picker(st: &mut AppState, tick: &[&str]) {
    crate::update::update(st, Msg::Groups(GroupMsg::AddDialog(0)), Instant::now());
    let Some(Modal::AddToGroup(p)) = &mut st.modal else { panic!("the picker opens") };
    p.ticked = tick.iter().map(|r| PathBuf::from(abs(r))).collect();
}

fn picker(theme: ThemeChoice, dark: bool, name: &str) {
    let mut st = picker_state(theme);
    open_picker(&mut st, &["docs"]);
    let mut h = harness(st, dark);
    h.run();
    for label in ["Add to Work", "Favorites", "Recent", "In Home", "Starred when added", "Add 1"] {
        assert!(h.query_all_by_label(label).next().is_some(), "{label} is shown");
    }
    assert!(h.query_by_label_contains("Filter").is_none(), "five rows need no filter");
    assert!(h.query_by_label("old-site").is_none(), "a missing Recent entry isn't offered");
    h.check_snapshot(name);
}

#[test]
fn repo_group_add_dialog() {
    picker(ThemeChoice::Dark, true, "repo_group_add_dialog");
}

#[test]
fn repo_group_add_dialog_light() {
    picker(ThemeChoice::Light, false, "repo_group_add_dialog_light");
}

#[test]
fn repo_group_add_dialog_ticks_and_adds_in_list_order() {
    let mut st = picker_state(ThemeChoice::Dark);
    open_picker(&mut st, &[]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Add").is_disabled(), "nothing ticked");
    // Tick api-server (from Home) by its row, then docs by its box's row.
    let at = centre(&h, "api-server");
    click(&mut h, at, PointerButton::Primary);
    let at = centre(&h, "docs");
    click(&mut h, at, PointerButton::Primary);
    h.run();
    h.get_by_label("Add 2").click();
    h.run();
    let want = vec![PathBuf::from(abs("docs")), PathBuf::from(api())];
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::AddRepos { group: 0, paths }) if *paths == want), "list order, not tick order");
}

#[test]
fn repo_group_add_dialog_filter() {
    let mut st = picker_state(ThemeChoice::Dark);
    st.recent.extend((0..5).map(|i| PathBuf::from(abs(&format!("tool-{i}")))));
    open_picker(&mut st, &["docs", "tool-3"]);
    if let Some(Modal::AddToGroup(p)) = &mut st.modal {
        p.filter = "tool".into();
    }
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("docs").is_none(), "the filter hides docs");
    assert!(h.query_by_label("Add 2").is_some(), "the hidden tick still counts");
    h.check_snapshot("repo_group_add_dialog_filter");
}

#[test]
fn repo_group_add_dialog_empty() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos = vec![web(), api()];
    let mut work = RepoGroup::new("Work", None);
    work.repos = vec![web(), api()];
    st.settings.repo_groups = vec![work];
    st.recent = vec![PathBuf::from(web())];
    open_picker(&mut st, &[]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Nothing to add: every favorite is already in this group and Recent is empty.").is_some());
    assert!(h.query_by_label("Cancel").is_none(), "Close only");
    h.check_snapshot("repo_group_add_dialog_empty");
}

#[test]
fn repo_group_add_dialog_german() {
    let mut st = picker_state(ThemeChoice::Dark);
    st.settings.language = "de".into();
    open_picker(&mut st, &["docs", "cli"]);
    let mut h = harness(st, true);
    h.run();
    for label in ["Zu Work hinzufügen", "In Home", "2 hinzufügen"] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    h.check_snapshot("repo_group_add_dialog_german");
}

/// Favorites web-app, api-server (both in Work, Rose) and docs; Home is an empty group.
fn welcome(theme: ThemeChoice) -> AppState {
    let mut st = welcome_state(theme);
    st.settings.favorite_repos = vec![web(), api(), abs("docs")];
    let mut work = RepoGroup::new("Work", Some(RepoColour::Rose));
    work.repos = vec![web(), api()];
    st.settings.repo_groups = vec![work, RepoGroup::new("Home", None)];
    st
}

fn tall(st: AppState, dark: bool) -> H {
    harness_sized(st, dark, vec2(1400.0, 1040.0))
}

#[test]
fn welcome_repo_groups_light() {
    let mut h = tall(welcome(ThemeChoice::Light), false);
    h.run();
    h.check_snapshot("welcome_repo_groups_light");
}

#[test]
fn the_empty_groups_link_opens_its_picker_and_hides_while_dragging() {
    let mut h = tall(welcome(ThemeChoice::Dark), true);
    h.run();
    assert_eq!(h.query_all_by_label("Add repositories…").count(), 1, "Home's placeholder only");
    let (link, empty) = (h.get_by_label("Add repositories…").bounding_box().unwrap(), h.get_by_label("No repositories yet"));
    let b = empty.bounding_box().unwrap();
    assert!(link.y0 >= b.y0 && link.y1 <= b.y1, "inside the dashed box");
    let open_all = h.get_all_by_label("Open all").next().unwrap().bounding_box().unwrap();
    assert!((link.x1 - open_all.x1).abs() < 0.5, "on the Open all column: {} {}", link.x1, open_all.x1);
    h.get_by_label("Add repositories…").click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::AddDialog(1))));
    // Dragging a favorite: the box is only a drop place.
    let grip = h.get_all_by_label("Drag to reorder").last().unwrap().bounding_box().unwrap();
    let start = pos2(((grip.x0 + grip.x1) / 2.0) as f32, ((grip.y0 + grip.y1) / 2.0) as f32);
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(start));
    events.push(Event::PointerButton { pos: start, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerMoved(start + vec2(0.0, -12.0)));
    h.step();
    h.run();
    assert!(h.query_by_label("Add repositories…").is_none(), "no link while dragging");
}

#[test]
fn welcome_group_menu_adds_repositories() {
    let mut h = tall(welcome(ThemeChoice::Dark), true);
    h.run();
    let at = centre(&h, "Group actions");
    click(&mut h, at, PointerButton::Primary);
    h.run();
    let rename = h.get_by_label("Rename…").bounding_box().unwrap();
    // The menu's item, just above Rename… (Home's placeholder has the other).
    let item = h
        .get_all_by_label("Add repositories…")
        .filter_map(|n| n.bounding_box())
        .find(|b| b.y1 <= rename.y0 + 0.5 && b.y0 > rename.y0 - 60.0)
        .expect("Add repositories… right above Rename…");
    click(&mut h, pos2(((item.x0 + item.x1) / 2.0) as f32, ((item.y0 + item.y1) / 2.0) as f32), PointerButton::Primary);
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::AddDialog(0))));
}

/// Opens old-site's (a Recent row's) menu and its Add to group submenu.
fn recent_menu(st: AppState) -> H {
    let mut h = tall(st, true);
    h.run();
    let at = centre(&h, &shown(&abs("old-site")));
    click(&mut h, at, PointerButton::Secondary);
    h.run();
    open_submenu(&mut h, "Add to group");
    h
}

#[test]
fn welcome_recent_add_to_group() {
    let mut h = recent_menu(welcome(ThemeChoice::Dark));
    for label in ["New group…", "Remove from recent"] {
        assert!(h.query_all_by_label(label).next().is_some(), "{label} is shown");
    }
    assert!(h.query_by_label("Not in a group").is_none(), "only groups to add to");
    h.check_snapshot("welcome_recent_add_to_group");
    h.get_all_by_label("Home").last().expect("Home in the submenu").click();
    h.run();
    let old = vec![PathBuf::from(abs("old-site"))];
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::AddRepos { group: 1, paths }) if *paths == old));
}

#[test]
fn welcome_recent_add_to_group_no_groups() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos = vec![web()];
    let mut h = recent_menu(st);
    h.check_snapshot("welcome_recent_add_to_group_no_groups");
    h.get_by_label("New group…").click();
    h.run();
    let old = Some(PathBuf::from(abs("old-site")));
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::NewDialog { repo }) if *repo == old));
}

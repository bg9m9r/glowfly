//! Repository groups on the welcome screen and in the toolbar's repository menu: the group
//! headers with their swatch, links and ⋯ menu, an empty group's line, a favorite's Move to group
//! submenu, dragging a favorite into a group, the groups in the menu, and the New group dialog.
//! The listed paths are labels only, never opened.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_repo_list::{H, abs, api, last, shown, web, welcome_state};
use super::snapshot_tests_repo_menu::open_repo_menu;
use super::snapshot_tests_select::click;
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::repo_groups::GroupForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{RepoColour, RepoGroup, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

fn docs() -> String {
    abs("docs")
}

/// Favorites web-app, api-server (both in Work, Rose) and docs; Home is an empty group.
fn grouped(st: &mut AppState) {
    st.settings.favorite_repos = vec![web(), api(), docs()];
    let mut work = RepoGroup::new("Work", Some(RepoColour::Rose));
    work.repos = vec![web(), api()];
    st.settings.repo_groups = vec![work, RepoGroup::new("Home", None)];
}

fn welcome() -> AppState {
    let mut st = welcome_state(ThemeChoice::Dark);
    grouped(&mut st);
    st
}

fn centre(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

#[test]
fn welcome_repo_groups() {
    let mut h = harness(welcome(), true);
    h.run();
    for label in ["Work", "Home", "Favorites", "Recent", "New group", "Drag a favorite here, or choose Move to group in its menu."] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert_eq!(h.query_all_by_label("Group actions").count(), 2, "a ⋯ per group");
    assert_eq!(h.query_all_by_label("Drag to reorder").count(), 3, "a grip per favorite");
    if cfg!(not(target_os = "macos")) {
        let note = "Drag to reorder or to move between groups. The first nine open with Ctrl+Alt+1 to Ctrl+Alt+9.";
        assert!(h.query_by_label(note).is_some(), "the note says groups take drops");
    }
    h.check_snapshot("welcome_repo_groups");
    h.get_by_label("New group").click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::NewDialog { repo: None })));
    let open_all = h.get_all_by_label("Open all").next().expect("Work's Open all").bounding_box().unwrap();
    click(&mut h, pos2(((open_all.x0 + open_all.x1) / 2.0) as f32, ((open_all.y0 + open_all.y1) / 2.0) as f32), PointerButton::Primary);
    let work = [PathBuf::from(web()), PathBuf::from(api())];
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::OpenAll(p)) if p == &work), "only the group's repositories");
}

#[test]
fn welcome_repo_group_menu() {
    let mut h = harness(welcome(), true);
    h.run();
    let more = h.get_all_by_label("Group actions").next().unwrap().bounding_box().unwrap();
    click(&mut h, pos2(((more.x0 + more.x1) / 2.0) as f32, ((more.y0 + more.y1) / 2.0) as f32), PointerButton::Primary);
    h.run();
    for label in ["Rename…", "Color", "Move up", "Move down", "Remove group"] {
        assert!(h.query_by_label(label).is_some(), "the group menu has {label}");
    }
    assert!(h.get_by_label("Move up").is_disabled(), "the first group can't move up");
    h.check_snapshot("welcome_repo_group_menu");
    h.get_by_label("Remove group").click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::Remove(0))));
}

#[test]
fn welcome_move_to_group() {
    let mut h = harness(welcome(), true);
    h.run();
    let row = centre(&h, &shown(&docs()));
    click(&mut h, row, PointerButton::Secondary);
    h.get_by_label("Move to group").hover();
    h.run();
    for label in ["Not in a group", "New group…"] {
        assert!(h.query_by_label(label).is_some(), "the submenu has {label}");
    }
    assert_eq!(h.query_all_by_label("Work").count(), 2, "the header and the submenu's item");
    h.check_snapshot("welcome_move_to_group");
    h.get_all_by_label("Home").last().expect("Home in the submenu").click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::MoveRepo { path, group: Some(1), .. }) if path == &PathBuf::from(docs())));
}

fn pointer(h: &mut H, pos: Pos2, button: Option<bool>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some(pressed) = button {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
}

#[test]
fn welcome_drag_into_an_empty_group() {
    let mut h = harness(welcome(), true);
    h.run();
    let grip = h.get_all_by_label("Drag to reorder").last().unwrap().bounding_box().unwrap();
    let start = pos2(((grip.x0 + grip.x1) / 2.0) as f32, ((grip.y0 + grip.y1) / 2.0) as f32);
    let over = centre(&h, "Drag a favorite here, or choose Move to group in its menu.");
    pointer(&mut h, start, Some(true));
    h.step();
    pointer(&mut h, start + vec2(0.0, -6.0), None);
    h.step();
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    pointer(&mut h, over, Some(false));
    h.run();
    let docs_moved = |p: &PathBuf| p == &PathBuf::from(docs());
    assert!(
        matches!(last(&h), Msg::Groups(GroupMsg::MoveRepo { path, group: Some(1), index: 0 }) if docs_moved(path)),
        "dropped into Home"
    );
}

#[test]
fn repo_menu_groups() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu-groups");
    grouped(&mut st);
    st.recent = vec![PathBuf::from(abs("old-site"))];
    let mut h = harness(st, true);
    h.run();
    open_repo_menu(&mut h);
    h.run();
    for label in ["Work", "Favorites", "Recent"] {
        assert!(h.query_by_label(label).is_some(), "{label} heads its rows");
    }
    assert!(h.query_by_label("Home").is_none(), "an empty group has no header");
    h.check_snapshot("repo_menu_groups");
    remove_fixture(&dir);
}

#[test]
fn repo_group_dialog() {
    let mut st = welcome();
    let mut form = GroupForm::new(&st.settings, None, Some(PathBuf::from(docs())));
    form.text = "work".into();
    st.modal = Some(Modal::RepoGroup(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("There's already a group called work.").is_some());
    assert!(h.get_by_label("Create").is_disabled(), "a name in use can't be taken");
    h.check_snapshot("repo_group_dialog");
    if let Some(Modal::RepoGroup(f)) = &mut h.state_mut().0.modal {
        f.text = "Clients".into();
    }
    h.run();
    h.get_by_label("Create").click();
    h.run();
    let made = |name: &str, repo: &Option<PathBuf>| name == "Clients" && repo.as_deref() == Some(std::path::Path::new(&docs()));
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::Create { name, repo }) if made(name, repo)));
    let (st, sent) = h.state_mut();
    let create = sent.pop().expect("the Create message");
    crate::update::update(st, create, Instant::now());
    h.run();
    assert!(h.state().0.modal.is_none(), "Create closes the dialog");
    assert!(h.query_by_label("Create").is_none(), "its Create button is gone");
    assert!(h.query_by_label("Clients").is_some(), "the start page lists the new group");
}

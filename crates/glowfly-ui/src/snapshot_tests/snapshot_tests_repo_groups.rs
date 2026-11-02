//! Repository groups on the start page and in the toolbar's repository menu: the groups nested
//! under Favorites with their headers (swatch, count, links and ⋯ menu), an empty group's
//! outlined placeholder, the line about groups while none has a member, a favorite's Move to
//! group submenu (from a right-click or ⋯), dragging a favorite into a group or out of the groups,
//! the groups in the menu, and the New group dialog. The listed paths are labels only, never
//! opened.

use super::menu_testing::open_submenu;
use super::snapshot_tests::{harness, harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_repo_list::{H, abs, api, hover_row, last, shown, web, welcome_state};
use super::snapshot_tests_repo_menu::open_repo_menu;
use super::snapshot_tests_select::click;
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::repo_groups::GroupForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
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

/// The start page in a window tall enough to show every row of `welcome`, Recent's included.
fn tall(st: AppState) -> H {
    harness_sized(st, true, vec2(1400.0, 1040.0))
}

fn centre(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

#[test]
fn welcome_repo_groups() {
    let mut h = tall(welcome());
    h.run();
    for label in ["Work", "Home", "Favorites", "Other favorites", "Recent", "New group", "No repositories yet"] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert_eq!(h.query_all_by_label("Group actions").count(), 2, "a ⋯ per group");
    assert_eq!(h.query_all_by_label("Drag to reorder").count(), 3, "a grip per favorite");
    assert!(h.query_by_label("Groups color their repositories' tabs and open together.").is_none(), "Work has members");
    let top = |h: &H, label: &str| h.get_by_label(label).bounding_box().expect(label).y0;
    assert!(top(&h, "Favorites") < top(&h, "Work") && top(&h, "Home") < top(&h, "Other favorites"), "the groups sit under Favorites");
    let left = |h: &H, label: &str| h.get_by_label(label).bounding_box().expect(label).x0;
    assert!(left(&h, &shown(&web())) > left(&h, &shown(&abs("old-site"))), "grouped rows are indented");
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
fn welcome_repo_row_hover() {
    let mut h = tall(welcome());
    h.run();
    // One frame on the row: its grip, ⋯ and hover layer, before the path's tooltip comes up.
    let row = centre(&h, &shown(&web()));
    h.input_mut().events.push(Event::PointerMoved(row));
    h.step();
    assert!(h.query_by_label("Repository actions").is_some(), "⋯ shows on the hovered row");
    h.check_snapshot("welcome_repo_row_hover");
}

#[test]
fn welcome_repo_group_menu() {
    let mut h = tall(welcome());
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
    let mut h = tall(welcome());
    h.run();
    // ⋯ on the row opens the same menu as a right-click.
    hover_row(&mut h, &shown(&docs()));
    let more = centre(&h, "Repository actions");
    click(&mut h, more, PointerButton::Primary);
    open_submenu(&mut h, "Move to group");
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
    let mut h = tall(welcome());
    h.run();
    let grip = h.get_all_by_label("Drag to reorder").last().unwrap().bounding_box().unwrap();
    let start = pos2(((grip.x0 + grip.x1) / 2.0) as f32, ((grip.y0 + grip.y1) / 2.0) as f32);
    let over = centre(&h, "No repositories yet");
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

#[test]
fn welcome_groups_intro_and_drop_out() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos = vec![web(), api()];
    let mut work = RepoGroup::new("Work", Some(RepoColour::Rose));
    work.repos = vec![web(), api()];
    st.settings.repo_groups = vec![work];
    let mut h = tall(st);
    h.run();
    assert!(h.query_by_label("Other favorites").is_none(), "every favorite is in a group");
    let grip = h.get_all_by_label("Drag to reorder").next().unwrap().bounding_box().unwrap();
    let start = pos2(((grip.x0 + grip.x1) / 2.0) as f32, ((grip.y0 + grip.y1) / 2.0) as f32);
    pointer(&mut h, start, Some(true));
    h.step();
    pointer(&mut h, start + vec2(0.0, 12.0), None);
    h.step();
    h.run();
    assert!(h.query_by_label("Drop here to take it out of its group.").is_some(), "a drag shows where the others go");
    let over = centre(&h, "Drop here to take it out of its group.");
    pointer(&mut h, over, None);
    h.step();
    pointer(&mut h, over, Some(false));
    h.run();
    let web_moved = |p: &PathBuf| p == &PathBuf::from(web());
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::MoveRepo { path, group: None, index: 0 }) if web_moved(path)), "out of Work");
    // A group without members: the line says what groups do.
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.repo_groups = vec![RepoGroup::new("Home", None)];
    let mut h = tall(st);
    h.run();
    assert!(h.query_by_label("Groups color their repositories' tabs and open together.").is_some());
    h.check_snapshot("welcome_groups_intro");
}

/// The right edge of the node labelled `label` (the `nth` of them, top first).
fn right(h: &H, label: &str, nth: usize) -> f32 {
    h.get_all_by_label(label).nth(nth).and_then(|n| n.bounding_box()).expect(label).x1 as f32
}

#[test]
fn welcome_header_actions_end_on_the_rows_more_column() {
    let mut h = tall(welcome());
    h.run();
    hover_row(&mut h, &shown(&web()));
    let edge = right(&h, "Repository actions", 0);
    for (label, nth) in [("New group", 0), ("Group actions", 0), ("Group actions", 1), ("Clear", 0)] {
        assert!((right(&h, label, nth) - edge).abs() < 0.5, "{label} ends on the rows' ⋯ column");
    }
    let (work, others) = (right(&h, "Open all", 0), right(&h, "Open all", 1));
    assert!((work - others).abs() < 0.5, "Work's and the others' Open all line up: {work} {others}");
}

#[test]
fn welcome_tab_from_a_row_reaches_a_shown_control_and_the_grip_never_opens() {
    let mut h = tall(welcome());
    h.run();
    h.get_by_label(&shown(&web())).focus();
    h.run();
    h.press_key(Key::Tab);
    h.run();
    let star = h.get_all_by_label("Remove from favorites").next().expect("web-app's star");
    assert!(star.is_focused(), "Tab from the row lands on its star, not on the hidden grip");
    h.press_key(Key::Tab);
    h.run();
    assert!(h.get_by_label("Repository actions").is_focused(), "then on its ⋯");
    // A click on the grip neither opens the repository nor sends anything else.
    let grip = h.get_all_by_label("Drag to reorder").next().unwrap().bounding_box().unwrap();
    let at = pos2(((grip.x0 + grip.x1) / 2.0) as f32, ((grip.y0 + grip.y1) / 2.0) as f32);
    h.state_mut().1.clear();
    click(&mut h, at, PointerButton::Primary);
    assert!(h.state().1.is_empty(), "a click on the grip opens nothing");
}

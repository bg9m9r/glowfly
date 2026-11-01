//! The welcome screen's Favorites and Recent sections (rows, icons, the row menus, "open" tags,
//! dragging favorites); the toolbar's repository menu is `snapshot_tests_repo_menu.rs`. Shares the renderer and fixture
//! repository with `snapshot_tests`. The listed paths are labels only, never opened.

use super::snapshot_tests::{harness, settings};
use super::snapshot_tests_select::click;
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::{AppState, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

/// A path that is absolute on every system (`C:/work/…` on Windows, `/work/…` elsewhere).
pub(crate) fn abs(rel: &str) -> String {
    if cfg!(windows) { format!("C:/work/{rel}") } else { format!("/work/{rel}") }
}

pub(super) fn api() -> String {
    abs("api-server")
}
pub(super) fn old() -> String {
    abs("old-site")
}
pub(super) fn web() -> String {
    abs("web-app")
}

/// Recent api-server, old-site (missing) and web-app, with web-app a favorite.
pub(super) fn lists(st: &mut AppState) {
    st.recent = [api(), old(), web()].iter().map(PathBuf::from).collect();
    st.settings.favorite_repos = vec![web()];
    st.missing_repos = vec![PathBuf::from(old())];
}

/// How the list shows `path`: in this system's spelling (`C:\work\web-app` on Windows).
pub(crate) fn shown(path: &str) -> String {
    glowfly_core::native_path::shown(std::path::Path::new(path))
}

pub(crate) fn welcome_state(theme: ThemeChoice) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    lists(&mut st);
    st
}

pub(super) type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The bounds of the node labelled `label` that is `small` (a painted icon, 18 px wide) or not,
/// on the same row as the `row` link.
fn bounds(h: &H, label: &str, row: &str, small: bool) -> Rect {
    let link = h.get_by_label(row).bounding_box().expect("the row's link");
    let b = h
        .get_all_by_label(label)
        .filter_map(|n| n.bounding_box())
        .find(|b| (b.x1 - b.x0 < 24.0) == small && (!small || (b.y0 + b.y1) / 2.0 > link.y0 && (b.y0 + b.y1) / 2.0 < link.y1))
        .unwrap_or_else(|| panic!("{label} for {row}"));
    Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

pub(super) fn last(h: &H) -> &Msg {
    h.state().1.last().expect("a message")
}

fn right_click(h: &mut H, label: &str) {
    let link = h.get_by_label(label).bounding_box().expect("the link");
    click(h, pos2(((link.x0 + link.x1) / 2.0) as f32, ((link.y0 + link.y1) / 2.0) as f32), PointerButton::Secondary);
}

#[test]
fn welcome_repos() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    assert_eq!(h.query_all_by_label(&shown(&web())).count(), 1, "a favorite is listed once");
    for label in ["Favorites", "Recent", "not found", "Open all", "Remove missing", &shown(&api()), &shown(&old())] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert!(h.query_by_label("Drag to reorder").is_none(), "one favorite has no grip");
    h.check_snapshot("welcome_repos");
    h.get_by_label("Remove missing").click();
    h.run();
    let Msg::Confirm { title, body, then, .. } = last(&h) else { panic!("Remove missing asks first") };
    assert_eq!(title, "Remove missing repositories");
    assert!(body.starts_with("Remove 1 repository that can't be found from the Recent list?"), "{body}");
    assert!(matches!(**then, Msg::Recent(RecentMsg::RemoveMissing)));
    h.get_by_label("Open all").click();
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::OpenAll(p)) if *p == [PathBuf::from(web())]));
}

#[test]
fn welcome_no_favorites_hint() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos.clear();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Star a repository to keep it here.").is_some());
    assert!(h.query_by_label("Open all").is_none());
    h.check_snapshot("welcome_favorites_hint");
}

#[test]
fn welcome_row_menu() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    right_click(&mut h, &shown(&api()));
    for label in ["Open", "Add to favorites", "Remove from recent", "Copy path"] {
        assert!(h.get_all_by_label(label).any(|n| n.bounding_box().is_some_and(|b| b.x1 - b.x0 > 30.0)), "{label} in the menu");
    }
    assert!(h.query_by_label("Move up").is_none(), "only favorites move");
    h.check_snapshot("welcome_repo_row_menu");
    let item = bounds(&h, "Remove from recent", &shown(&api()), false);
    click(&mut h, item.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == &PathBuf::from(api())));
}

/// Three favorites: web-app, api-server, docs.
fn three_favorites() -> AppState {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos = vec![web(), api(), abs("docs")];
    st
}

#[test]
fn welcome_favorite_row_menu() {
    let mut h = harness(three_favorites(), true);
    h.run();
    right_click(&mut h, &shown(&web()));
    h.check_snapshot("welcome_favorite_row_menu");
    let disabled = |h: &H, label: &str| h.get_by_label(label).is_disabled();
    assert!(disabled(&h, "Move up"), "the first can't move up");
    assert!(!disabled(&h, "Move down"));
    h.get_by_label("Move down").click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::MoveRepo { path, group: None, index: 1 }) if path == &PathBuf::from(web())));
}

fn pointer(h: &mut H, pos: Pos2, button: Option<bool>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some(pressed) = button {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
}

#[test]
fn welcome_favorites_drag() {
    let mut h = harness(three_favorites(), true);
    h.run();
    if cfg!(not(target_os = "macos")) {
        assert!(h.query_by_label("Drag to reorder. The first nine open with Ctrl+Alt+1 to Ctrl+Alt+9.").is_some());
    }
    let grips: Vec<Rect> = h
        .get_all_by_label("Drag to reorder")
        .filter_map(|n| n.bounding_box())
        .map(|b| Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32)))
        .collect();
    assert_eq!(grips.len(), 3, "a grip per favorite");
    let start = grips[1].center();
    pointer(&mut h, start, Some(true));
    h.step();
    pointer(&mut h, start + vec2(0.0, -6.0), None);
    h.step();
    let over = pos2(start.x, grips[0].top() - 2.0);
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    h.check_snapshot("welcome_favorites_drag");
    pointer(&mut h, over, Some(false));
    h.run();
    let moved = |p: &PathBuf| p == &PathBuf::from(api());
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::MoveRepo { path, group: None, index: 0 }) if moved(path)), "moved above the first");
}

#[test]
fn welcome_repos_open_marker() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos = vec![web(), api()];
    // web-app is in the second tab; the first, empty one shows the welcome screen.
    st.tabs = vec![Tab::empty(), Tab::unloaded(PathBuf::from(web()))];
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.query_all_by_label("open").count(), 1, "web-app is open");
    assert!(h.query_by_label("Open all").is_some(), "api-server isn't");
    h.check_snapshot("welcome_repos_open_marker");
    h.get_by_label("open").click();
    h.run();
    assert!(matches!(last(&h), Msg::Repo(crate::msg::RepoMsg::Open(p)) if p.as_path() == std::path::Path::new(&web())));
}

#[test]
fn welcome_icons_push_messages() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    assert_eq!(h.query_all_by_label("Remove from recent").count(), 2, "only Recent rows have ×");
    let star = bounds(&h, "Add to favorites", &shown(&api()), true);
    click(&mut h, star.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if path == &PathBuf::from(api())));
    let star = bounds(&h, "Remove from favorites", &shown(&web()), true);
    click(&mut h, star.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: false }) if path == &PathBuf::from(web())));
    let cross = bounds(&h, "Remove from recent", &shown(&old()), true);
    click(&mut h, cross.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == &PathBuf::from(old())));
    h.get_by_label("Clear").click();
    h.run();
    let Msg::Confirm { title, body, then, .. } = last(&h) else { panic!("Clear asks first") };
    assert_eq!(title, "Clear recent repositories");
    assert!(body.contains("Remove all 3 repositories"), "{body}");
    assert!(matches!(**then, Msg::Recent(RecentMsg::ClearRecent)));
}

//! The tab bar to the keyboard and to screen readers: every repository tab is a Tab node named
//! after it and selected while active, its close × is named "Close tab {name}" and takes clicks
//! on a 24 x 24 area, and Tab walks the drawn title bar's menu button, the tabs, their × and
//! the + and the tab list button (`tabs_close_focus`: the ring on an inactive tab's ×). A
//! middle-click on the × closes its tab and a right-click opens the tab's menu. Preferences'
//! navigation rows take the focus too.

use super::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use super::tabs_tests::pointer;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, accesskit::Role, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{Msg, TabsMsg};
use glowfly_model::state::menu_bar::tab_name;
use glowfly_model::state::tabs::ClosedTab;
use glowfly_model::state::window_chrome::Chrome;
use glowfly_model::state::{PrefPage, PrefsState};
use std::path::PathBuf;

#[test]
fn tabs_are_tab_nodes_and_each_close_names_its_tab() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "tabs-access");
    let names: Vec<String> = (0..st.tabs.len()).map(|i| tab_name(&st, i).to_owned()).collect();
    // A tab is named with its states (design pass D15); its × by the name alone.
    let t = st.settings.lang();
    let labels: Vec<String> = names.iter().enumerate().map(|(i, n)| st.tab_glance(i).describe(t, n)).collect();
    assert!(labels[0].starts_with(&format!("{}: ", names[0])), "the fixture has changes: {}", labels[0]);
    let active = st.active_tab;
    let mut h = harness_sized(st, true, vec2(1400.0, 820.0));
    h.run();
    let tabs: Vec<_> = h.query_all_by_role(Role::Tab).collect();
    assert_eq!(tabs.len(), names.len(), "one Tab node per tab");
    let list = h.get_by_role(Role::TabList);
    assert_eq!(list.query_all_by_role(Role::Tab).count(), names.len(), "every tab in the one tab list");
    for (i, name) in names.iter().enumerate() {
        let tab = h.query_all_by_role(Role::Tab).find(|n| n.label().as_deref() == Some(labels[i].as_str())).expect("named after the tab");
        assert_eq!(tab.is_selected(), Some(i == active), "{name}: selected only while active");
        let close_name = format!("Close tab {name}");
        let close = h.get_by_label(&close_name);
        assert_eq!(close.role(), Role::Button);
        let b = close.bounding_box().expect("bounds");
        assert_eq!(((b.x1 - b.x0) as f32, (b.y1 - b.y0) as f32), (24.0, 24.0), "{name}: a 24 x 24 hit area");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_walks_the_title_bar_and_its_icon_buttons() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-focus");
    st.view.chrome = Chrome::Windows;
    st.settings.reduce_motion = true;
    // A closed tab, so the tab list button shows.
    st.closed_tabs = vec![ClosedTab::from(PathBuf::from("C:/work/old-repo"))];
    let first = tab_name(&st, 0).to_owned();
    let first_label = st.tab_glance(0).describe(st.settings.lang(), &first);
    let mut h = harness_sized(st, true, vec2(1440.0, 880.0));
    h.run();
    let mut reached = Vec::new();
    for _ in 0..10 {
        h.press_key(Key::Tab);
        h.run();
        if let Some(n) = h.query_all(egui_kittest::kittest::by()).find(|n| n.is_focused()) {
            reached.push(n.label().unwrap_or_default());
        }
    }
    for want in ["Menu", first_label.as_str(), &format!("Close tab {first}"), "New tab", "All tabs and recently closed tabs"] {
        assert!(reached.iter().any(|l| l == want), "Tab reaches {want:?}: {reached:?}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_walks_the_preferences_navigation_rows() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "prefs-nav-focus");
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = harness_sized(st, true, vec2(1400.0, 820.0));
    h.run();
    let mut rows = Vec::new();
    for _ in 0..6 {
        h.press_key(Key::Tab);
        h.run();
        let id = h.ctx.memory(|m| m.focused()).expect("Tab focuses something");
        let at = h.ctx.read_response(id).map(|r| r.rect).expect("a drawn target");
        // The navigation column is 250 px wide, its rows the column's full width.
        if at.left() <= 0.5 && (at.width() - 250.0).abs() <= 1.0 {
            rows.push(id);
        }
    }
    rows.dedup();
    assert!(rows.len() >= 4, "Tab moves down the navigation rows: {rows:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tabs_close_focus() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-close-focus");
    st.settings.reduce_motion = true;
    let mut h = harness_sized(st, true, vec2(1400.0, 820.0));
    h.run();
    let want = "Close tab api-server";
    let mut on = false;
    for _ in 0..12 {
        h.press_key(Key::Tab);
        h.run();
        if h.query_all_by_label(want).any(|n| n.is_focused()) {
            on = true;
            break;
        }
    }
    assert!(on, "Tab reaches {want}");
    h.check_snapshot("tabs_close_focus");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_close_cross_middle_closes_and_right_clicks_the_tab_menu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "tabs-close-clicks");
    let mut h = harness_sized(st, true, vec2(1400.0, 820.0));
    h.run();
    let b = h.get_by_label("Close tab api-server").bounding_box().expect("bounds");
    let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    pointer(&mut h, at, Some((PointerButton::Middle, true)));
    h.step();
    pointer(&mut h, at, Some((PointerButton::Middle, false)));
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::CloseTab(1)))), "a middle-click closes its tab");
    h.state_mut().1.clear();
    pointer(&mut h, at, Some((PointerButton::Secondary, true)));
    h.step();
    pointer(&mut h, at, Some((PointerButton::Secondary, false)));
    h.run();
    h.get_by_label("Close other tabs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::CloseOtherTabs(1)))), "a right-click opens its tab's menu");
    drop(h);
    remove_fixture(&dir);
}

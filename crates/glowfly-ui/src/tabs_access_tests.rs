//! The tab bar to the keyboard and to screen readers: every repository tab is a Tab node named
//! after it and selected while active, its close × is named "Close tab {name}" and takes clicks
//! on a 24 x 24 area, and Tab walks the drawn title bar's menu button, the tabs, their × and
//! the + and the tab list button. Preferences' navigation rows take the focus too.

use super::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use crate::state::menu_bar::tab_name;
use crate::state::tabs::ClosedTab;
use crate::state::window_chrome::Chrome;
use crate::state::{PrefPage, PrefsState};
use eframe::egui::{Key, accesskit::Role, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

#[test]
fn tabs_are_tab_nodes_and_each_close_names_its_tab() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "tabs-access");
    let names: Vec<String> = (0..st.tabs.len()).map(|i| tab_name(&st, i).to_owned()).collect();
    let active = st.active_tab;
    let mut h = harness_sized(st, true, vec2(1400.0, 820.0));
    h.run();
    let tabs: Vec<_> = h.query_all_by_role(Role::Tab).collect();
    assert_eq!(tabs.len(), names.len(), "one Tab node per tab");
    for (i, name) in names.iter().enumerate() {
        let tab = h.query_all_by_role(Role::Tab).find(|n| n.label().as_deref() == Some(name)).expect("named after the tab");
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
    for want in ["Menu", first.as_str(), &format!("Close tab {first}"), "New tab", "All tabs and recently closed tabs"] {
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

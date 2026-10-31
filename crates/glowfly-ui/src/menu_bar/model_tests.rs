//! The drawn menus come from the menu model (`state/menu_model.rs`): the items that change view
//! state do it when clicked, and a drawn title bar's menus and Preferences leave out the menu
//! bar, which only the system frame has.

use super::snapshot_tests::with_bar;
use crate::msg::{Msg, SearchMsg};
use crate::state::window_chrome::Chrome;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::menu_testing::open_submenu;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use egui_kittest::Harness;
use egui_kittest::kittest::{Node, Queryable};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The widest node labelled `label`: the open menu's item rather than a button of that name.
fn widest<'a>(h: &'a H, label: &'a str) -> Node<'a> {
    let width = |n: &Node<'_>| n.bounding_box().map_or(0.0, |b| b.x1 - b.x0);
    h.query_all_by_label(label).max_by(|a, b| width(a).total_cmp(&width(b))).unwrap_or_else(|| panic!("{label}"))
}

/// Opens menu `title` on the system frame's bar and clicks `item`.
fn click(st: AppState, title: &str, item: &str) -> H {
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(title).click();
    h.run();
    widest(&h, item).click();
    h.run();
    h
}

#[test]
fn drawn_menus_come_from_the_model() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-model-activity");
    assert!(!st.view.show_log);
    let h = click(st, "View", "Activity log");
    assert!(h.state().0.view.show_log, "View > Activity log shows the log");
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-model-filter");
    let h = click(st, "Edit", "Show only matching commits");
    assert!(h.state().0.view.show_search, "the find bar shows");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Search(SearchMsg::ToggleFilter))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn macos_menus_and_preferences_have_no_menu_bar() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "menu-model-mac");
    st.view.chrome = Chrome::MacOs;
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Menu").click();
    h.run();
    open_submenu(&mut h, "View");
    assert!(h.query_by_label("Full screen").is_some(), "the View submenu is open");
    assert!(h.query_by_label(Lang::En.tr("menu.view.menu_bar")).is_none());
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "menu-model-mac-prefs");
    st.view.chrome = Chrome::MacOs;
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(Lang::En.tr("prefs.ui.menu_bar.label")).is_none());
    assert!(h.query_by_label(Lang::En.tr("prefs.ui.menu_bar.check")).is_none());
    drop(h);
    remove_fixture(&dir);
}

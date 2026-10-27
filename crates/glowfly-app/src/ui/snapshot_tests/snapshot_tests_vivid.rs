//! Vivid colour: whole windows in every built-in theme over a merge stopped on a conflict, with
//! another file changed beside it (the window's top line and the tab swatches in the repository colours, the toolbar's neutral
//! backing, the coral banner and conflicted row, the neutral change bars), the drawn title bar,
//! the tab menu's Color submenu and the welcome screen's swatches, and what the Color submenus
//! send.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, harness_palette, harness_sized, remove_fixture, settings, with_conflict};
use super::snapshot_tests_repo_list::{abs, shown};
use super::snapshot_tests_select::click;
use super::tabs_tests::{pointer, tab_centre};
use super::theme::{self, color::Palette};
use crate::msg::{Msg, RecentMsg};
use crate::state::window_chrome::Chrome;
use crate::state::{AppState, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::{RepoColour, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The three sides of a settings file whose third line both sides changed.
fn sides(side: &str) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=8).map(|i| format!("setting_{i} = {i}")).collect();
    match side {
        "ours" => lines[2] = "theme = \"dark\"".into(),
        "theirs" => lines[2] = "theme = \"solarized\"".into(),
        _ => {}
    }
    (lines.join("\n") + "\n").into_bytes()
}

/// A merge stopped on a conflict, with app.txt changed beside it and the WIP panel showing
/// (the conflict view closed); the repository is coloured Teal, the fixture's second tab is
/// uncoloured and a third tab coloured Plum.
fn coloured(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_conflict(name, "settings.txt", sides);
    st.session.conflict.view = None;
    std::fs::write(
        dir.join("app.txt"),
        "app
fix
one more line
and another
",
    )
    .unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let generation = st.session.reads.line_counts.start_wip().unwrap();
    st.session.reads.line_counts.finish_wip(generation, Some(repo::wip_line_counts(&dir).unwrap()));
    st.settings.set_repo_colour(&dir, Some(RepoColour::Teal));
    let third = glowfly_core::paths::temp_dir().join("glowfly-vivid-tabs").join("docs");
    st.settings.set_repo_colour(&third, Some(RepoColour::Plum));
    st.tabs.push(Tab::unloaded(third));
    (st, dir)
}

pub(in crate::ui) fn whole(name: &str, p: Palette) {
    let (st, dir) = coloured(name);
    let mut h = harness_palette(st, p);
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn vivid_dark() {
    whole("vivid_dark", theme::DARK);
}

#[test]
fn vivid_light() {
    whole("vivid_light", theme::LIGHT);
}

#[test]
fn vivid_high_contrast_dark() {
    whole("vivid_high_contrast_dark", theme::HIGH_CONTRAST_DARK);
}

#[test]
fn vivid_high_contrast_light() {
    whole("vivid_high_contrast_light", theme::HIGH_CONTRAST_LIGHT);
}

#[test]
fn vivid_garden_dark() {
    whole("vivid_garden_dark", theme::GARDEN_DARK);
}

#[test]
fn vivid_garden_light() {
    whole("vivid_garden_light", theme::GARDEN_LIGHT);
}

#[test]
fn vivid_sand() {
    whole("vivid_sand", theme::SAND);
}

#[test]
fn vivid_title_bar() {
    let (mut st, dir) = coloured("vivid-title-bar");
    st.view.chrome = Chrome::Windows;
    st.settings.reduce_motion = true;
    let mut h = harness_sized(st, true, vec2(1440.0, 880.0));
    h.run();
    h.check_snapshot("vivid_title_bar");
    drop(h);
    remove_fixture(&dir);
}

fn right_click_tab(h: &mut H, i: usize) {
    let at = tab_centre(h, i);
    pointer(h, at, Some((PointerButton::Secondary, true)));
    h.step();
    pointer(h, at, Some((PointerButton::Secondary, false)));
    h.run();
}

/// The last colour a Color submenu sent.
fn sent(h: &H) -> Option<(PathBuf, Option<RepoColour>)> {
    h.state().1.iter().rev().find_map(|m| match m {
        Msg::Recent(RecentMsg::SetColour { path, colour }) => Some((path.clone(), *colour)),
        _ => None,
    })
}

#[test]
fn tab_colour_menu() {
    let (st, dir) = coloured("vivid-tab-menu");
    let mut h = harness(st, true);
    h.run();
    right_click_tab(&mut h, 0);
    open_submenu(&mut h, "Color");
    submenu_item(&h, "Color", "None");
    for hue in ["Rose", "Ochre", "Fern", "Sky", "Slate", "Iris", "Plum"] {
        submenu_item(&h, "Color", hue);
    }
    submenu_item(&h, "Color", "✔ Teal");
    h.check_snapshot("tab_colour_menu");
    submenu_item(&h, "Color", "None").click();
    h.run();
    assert_eq!(sent(&h), Some((dir.clone(), None)), "None takes the colour away");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_menu_color_sets_the_colour() {
    let (st, dir) = coloured("vivid-tab-menu-set");
    let second = st.tabs[1].path.clone().unwrap();
    let mut h = harness(st, true);
    h.run();
    right_click_tab(&mut h, 1);
    open_submenu(&mut h, "Color");
    submenu_item(&h, "Color", "✔ None");
    submenu_item(&h, "Color", "Plum").click();
    h.run();
    assert_eq!(sent(&h), Some((second, Some(RepoColour::Plum))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_new_tabs_color_item_is_blocked() {
    let (mut st, dir) = coloured("vivid-new-tab");
    st.tabs.push(Tab::empty());
    let mut h = harness(st, true);
    h.run();
    right_click_tab(&mut h, 3);
    let item = h.get_by_label("Color");
    assert!(item.is_disabled());
    assert_eq!(item.description().as_deref(), Some(glowfly_core::i18n::Lang::En.tr("repo_colour.blocked")));
    item.click();
    h.run();
    assert_eq!(sent(&h), None, "a blocked item sends nothing");
    drop(h);
    remove_fixture(&dir);
}

/// Two favorites and two recent entries, the first of each coloured.
fn welcome() -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    st.settings.favorite_repos = vec![abs("web-app"), abs("docs")];
    st.recent = [abs("api-server"), abs("old-site")].iter().map(PathBuf::from).collect();
    st.settings.set_repo_colour(&PathBuf::from(abs("web-app")), Some(RepoColour::Iris));
    st.settings.set_repo_colour(&PathBuf::from(abs("api-server")), Some(RepoColour::Ochre));
    st
}

#[test]
fn welcome_repo_colours() {
    let mut h = harness(welcome(), false);
    h.run();
    // Every row keeps the swatch's slot, so the paths line up.
    let left = |label: &str| h.get_by_label(&shown(&abs(label))).bounding_box().expect("the row").x0;
    assert_eq!(left("web-app"), left("docs"));
    assert_eq!(left("api-server"), left("old-site"));
    h.check_snapshot("welcome_repo_colours");
}

#[test]
fn welcome_row_color_sets_the_colour() {
    let mut h = harness(welcome(), false);
    h.run();
    let link = h.get_by_label(&shown(&abs("old-site"))).bounding_box().expect("the link");
    click(&mut h, pos2(((link.x0 + link.x1) / 2.0) as f32, ((link.y0 + link.y1) / 2.0) as f32), PointerButton::Secondary);
    open_submenu(&mut h, "Color");
    submenu_item(&h, "Color", "Teal").click();
    h.run();
    assert_eq!(sent(&h), Some((PathBuf::from(abs("old-site")), Some(RepoColour::Teal))));
}

//! The tab list at the end of the tab bar: shown only when there are closed tabs (or too many
//! tabs to fit), its popup of open and recently closed tabs, and what its rows send.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::tabs::ClosedTab;
use crate::state::{AppState, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;

const TIP: &str = "All tabs and recently closed tabs";

fn place(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-tab-list").join(name)
}

/// The fixture's two tabs, a third one labelled "Frontend", and two closed tabs (the newer one
/// labelled).
fn with_list(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.tabs.push(Tab { label: Some("Frontend".into()), ..Tab::unloaded(place("web")) });
    st.closed_tabs = vec![ClosedTab::from(place("old-repo")), ClosedTab { path: place("notes"), label: Some("Team notes".into()) }];
    (st, dir)
}

fn open_list(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    h.get_by_label(TIP).click();
    h.run();
}

/// Clicks the row labelled `label` with the mouse, as the popup's close-on-click expects.
fn click_row(h: &mut Harness<'_, (AppState, Vec<Msg>)>, label: &str) {
    let r = h.get_by_label(label).bounding_box().expect("the row");
    let at = pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32);
    super::snapshot_tests_select::click(h, at, PointerButton::Primary);
}

#[test]
fn tabs_list() {
    let (st, dir) = with_list("tabs-list");
    let mut h = harness(st, true);
    h.run();
    open_list(&mut h);
    h.check_snapshot("tabs_list");

    click_row(&mut h, "old-repo");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::ReopenClosed(p)) if p == &place("old-repo"))));
    assert!(h.query_by_label("Team notes").is_none(), "a click closes the list");

    h.state_mut().1.clear();
    open_list(&mut h);
    click_row(&mut h, "Frontend");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::SelectTab(2)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_list_shows_only_when_it_adds_something() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "tabs-list-hidden");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(TIP).is_none(), "two tabs that fit and nothing closed");
    drop(h);

    // Too many tabs to show at full width.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "tabs-list-hidden");
    st.tabs.extend((0..6).map(|i| Tab::unloaded(place(&format!("t{i}")))));
    let mut h = harness(st, true);
    h.run();
    open_list(&mut h);
    assert!(h.query_by_label("No closed tabs to reopen").is_some());
    drop(h);
    remove_fixture(&dir);
}

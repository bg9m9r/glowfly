//! Preferences' navigation column in a short window (1400 x 480): the page list scrolls under
//! Back and the search, the open page's row comes into view when the page changes, and the wheel
//! over the column scrolls the column, not the page.

use super::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_gitflow::center;
use super::snapshot_tests_sparse::sparse_page;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::sparse_ops::SparseStatus;
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::{AppState, PrefPage, PrefsState};
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const SHORT: eframe::egui::Vec2 = vec2(1400.0, 480.0);
/// How tall the list's bottom fade is: a row that is in view clears it.
const FADE_H: f64 = 24.0;

/// Where the list starts: the bottom of the search field above it.
fn list_top(h: &H) -> f64 {
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search preferences").bounding_box().expect("the field").y1
}

/// A nav row's bounds. Looked up by role, as "Sparse checkout" is also the open page's title.
fn row(h: &H, label: &str) -> egui::accesskit::Rect {
    h.get_by_role_and_label(egui::accesskit::Role::Button, label).bounding_box().expect("the row")
}

/// Where the page's "Updates" title is.
fn updates_y(h: &H) -> f64 {
    h.query_all_by_label("Updates").filter_map(|n| n.bounding_box()).map(|b| b.y0).fold(f64::MAX, f64::min)
}

fn in_view(h: &H, label: &str) -> bool {
    let b = row(h, label);
    b.y0 >= list_top(h) - 0.5 && b.y1 <= f64::from(SHORT.y) - FADE_H + 0.5
}

fn wheel(h: &mut H, at: Pos2, dy: f32) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, dy), modifiers: Modifiers::NONE });
    h.run_steps(6);
}

fn press(h: &mut H, at: Pos2) {
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
}

/// Runs the messages the view pushed through update.
fn apply(h: &mut H) {
    let (st, msgs) = h.state_mut();
    for m in std::mem::take(msgs) {
        glowfly_model::update::update(st, m, Instant::now());
    }
}

/// The fixture repository with Preferences open on General, in the short window.
fn general_short(name: &str) -> (H, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = harness_sized(st, true, SHORT);
    h.run();
    (h, dir)
}

#[test]
fn prefs_nav_short() {
    let (st, dir) = sparse_page(ThemeChoice::Dark, "prefs-nav-short", SparseStatus { tracked: 340, ..Default::default() });
    let mut h = harness_sized(st, true, SHORT);
    h.run();
    assert!(in_view(&h, "Sparse checkout"), "the open page's row is in view");
    assert!(row(&h, "General").y1 <= list_top(&h) + 0.5, "the list scrolled past General");
    let top = list_top(&h);
    assert!(h.get_by_label("Back to repository").bounding_box().expect("back").y0 < top);
    assert!(h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search preferences").bounding_box().expect("field").y0 < top);
    h.check_snapshot("prefs_nav_short");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_last_repository_row_can_be_reached_and_clicked_in_a_short_window() {
    let (mut h, dir) = general_short("prefs-nav-click");
    assert!(row(&h, "Sparse checkout").y0 >= f64::from(SHORT.y), "starts below the view");
    let updates = updates_y(&h);
    wheel(&mut h, pos2(125.0, 300.0), -2000.0);
    assert!(in_view(&h, "Sparse checkout"), "the wheel over the column scrolls it");
    assert!((updates_y(&h) - updates).abs() < 0.5, "the page did not scroll");
    h.state_mut().1.clear();
    let at = center(&h, "Sparse checkout");
    press(&mut h, at);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::Navigate(PrefPage::RepoSparse)))));
    apply(&mut h);
    h.run();
    assert_eq!(h.state().0.prefs.as_ref().unwrap().page, PrefPage::RepoSparse);
    assert!(in_view(&h, "Sparse checkout"), "no jump on opening");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn opening_a_page_brings_its_row_into_view_once() {
    let (mut h, dir) = general_short("prefs-nav-once");
    h.state_mut().1.push(Msg::Prefs(PrefsMsg::Navigate(PrefPage::RepoGitFlow)));
    apply(&mut h);
    h.run();
    assert!(in_view(&h, "Git Flow"));
    wheel(&mut h, pos2(125.0, 300.0), 2000.0);
    assert!(in_view(&h, "General"), "scrolled back up");
    assert!(row(&h, "Git Flow").y0 >= f64::from(SHORT.y), "not pulled back");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_wheel_over_the_page_leaves_the_column() {
    let (mut h, dir) = general_short("prefs-nav-page-wheel");
    let general = row(&h, "General").y0;
    wheel(&mut h, pos2(700.0, 300.0), -400.0);
    assert!((row(&h, "General").y0 - general).abs() < 0.5, "the column did not move");
    assert!(row(&h, "Sparse checkout").y0 >= f64::from(SHORT.y), "still below the view");
    drop(h);
    remove_fixture(&dir);
}

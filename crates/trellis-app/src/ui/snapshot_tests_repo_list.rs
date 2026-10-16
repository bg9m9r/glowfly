//! The welcome screen's Favorites and Recent sections (rows, icons, the row menu) and the
//! toolbar's repository menu. Shares the renderer and fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, settings, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::{Msg, RecentMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, Rect, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;

const API: &str = "C:/work/api-server";
const OLD: &str = "C:/work/old-site";
const WEB: &str = "C:/work/web-app";

/// Recent api-server, old-site (missing) and web-app, with web-app a favorite.
fn lists(st: &mut AppState) {
    st.recent = [API, OLD, WEB].iter().map(PathBuf::from).collect();
    st.settings.favorite_repos = vec![WEB.into()];
    st.missing_repos = vec![PathBuf::from(OLD)];
}

/// How the list shows `path`: in this system's spelling (`C:\work\web-app` on Windows).
fn shown(path: &str) -> String {
    trellis_core::native_path::shown(std::path::Path::new(path))
}

fn welcome_state(theme: ThemeChoice) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    lists(&mut st);
    st
}

type H = Harness<'static, (AppState, Vec<Msg>)>;

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

fn last(h: &H) -> &Msg {
    h.state().1.last().expect("a message")
}

#[test]
fn welcome_repos() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    assert_eq!(h.query_all_by_label(&shown(WEB)).count(), 1, "a favorite is listed once");
    for label in ["Favorites", "Recent", "not found", &shown(API), &shown(OLD)] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    h.check_snapshot("welcome_repos");
}

#[test]
fn welcome_repos_light() {
    render("welcome_repos_light", welcome_state(ThemeChoice::Light), false);
}

#[test]
fn welcome_no_favorites_hint() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.favorite_repos.clear();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Star a repository to keep it here.").is_some());
    h.check_snapshot("welcome_favorites_hint");
}

#[test]
fn welcome_row_menu() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    let link = h.get_by_label(&shown(API)).bounding_box().expect("the link");
    click(&mut h, pos2(((link.x0 + link.x1) / 2.0) as f32, ((link.y0 + link.y1) / 2.0) as f32), PointerButton::Secondary);
    for label in ["Open", "Add to favorites", "Remove from recent", "Copy path"] {
        assert!(h.get_all_by_label(label).any(|n| n.bounding_box().is_some_and(|b| b.x1 - b.x0 > 30.0)), "{label} in the menu");
    }
    h.check_snapshot("welcome_repo_row_menu");
    let item = bounds(&h, "Remove from recent", &shown(API), false);
    click(&mut h, item.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == &PathBuf::from(API)));
}

#[test]
fn welcome_icons_push_messages() {
    let mut h = harness(welcome_state(ThemeChoice::Dark), true);
    h.run();
    assert_eq!(h.query_all_by_label("Remove from recent").count(), 2, "only Recent rows have ×");
    let star = bounds(&h, "Add to favorites", &shown(API), true);
    click(&mut h, star.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if path == &PathBuf::from(API)));
    let star = bounds(&h, "Remove from favorites", &shown(WEB), true);
    click(&mut h, star.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: false }) if path == &PathBuf::from(WEB)));
    let cross = bounds(&h, "Remove from recent", &shown(OLD), true);
    click(&mut h, cross.center(), PointerButton::Primary);
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::Remove(p)) if p == &PathBuf::from(OLD)));
    h.get_by_label("Clear").click();
    h.run();
    let Msg::Confirm { title, body, then, .. } = last(&h) else { panic!("Clear asks first") };
    assert_eq!(title, "Clear recent repositories");
    assert!(body.contains("Remove all 3 repositories"), "{body}");
    assert!(matches!(**then, Msg::Recent(RecentMsg::ClearRecent)));
}

#[test]
fn repo_menu_favorites_and_recent() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "repo-menu");
    lists(&mut st);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let mut h = harness(st, true);
    h.run();
    // The repository name under the toolbar's "repository" caption opens the menu.
    let caption = h.get_by_label("repository").bounding_box().expect("the caption");
    let name = h
        .get_all_by_label("demo")
        .filter_map(|n| n.bounding_box())
        .filter(|b| b.y0 >= caption.y1 - 2.0 && (b.x0 - caption.x0).abs() < 20.0)
        .min_by(|a, b| a.y0.total_cmp(&b.y0))
        .expect("the repository button");
    click(&mut h, pos2(((name.x0 + name.x1) / 2.0) as f32, ((name.y0 + name.y1) / 2.0) as f32), PointerButton::Primary);
    for label in ["Add this repository to favorites", "Favorites", "Recent", "Clear recent", &shown(WEB), &shown(API)] {
        assert!(h.query_by_label(label).is_some(), "{label} is in the menu");
    }
    assert_eq!(h.query_all_by_label(&shown(WEB)).count(), 1, "a favorite is listed once");
    h.check_snapshot("repo_menu");
    h.get_by_label("Add this repository to favorites").click();
    h.run();
    assert!(matches!(last(&h), Msg::Recent(RecentMsg::SetFavorite { path, favorite: true }) if *path == workdir));
    remove_fixture(&dir);
}

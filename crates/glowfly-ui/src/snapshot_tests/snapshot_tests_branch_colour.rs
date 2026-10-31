//! Branch colours on every surface: the graph's lanes, nodes, bands and chips (main in the
//! accent, side-by-side branches in different hues, a remote chip on main's line in its own
//! branch's colour), the sidebar's dots (with a hidden branch's fade over its dot), the Branch
//! picker's menu, and the commit details' branch row and avatar ring.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::{GraphFilter, ThemeChoice};
use std::path::{Path, PathBuf};

const D: &str = "2026-01-12T10:00:00+00:00";

/// The fixture with topic and fix-1 (whose names share a hue) one commit each on top of main,
/// and origin with main and topic at main's tip.
pub(crate) fn with_branches(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    for (b, date) in [("topic", "2026-01-11T10:00:00+00:00"), ("fix-1", D)] {
        git(&dir, &["checkout", "-q", "-b", b, "main"], date);
        std::fs::write(dir.join(format!("{b}.txt")), b).unwrap();
        git(&dir, &["add", &format!("{b}.txt")], date);
        git(&dir, &["commit", "-q", "-m", &format!("Work on {b}")], date);
    }
    git(&dir, &["checkout", "-q", "main"], D);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/demo.git"], D);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "main"], D);
    git(&dir, &["update-ref", "refs/remotes/origin/topic", "main"], D);
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

fn tip(dir: &Path, branch: &str) -> git2::Oid {
    git2::Repository::open(dir).unwrap().refname_to_id(&format!("refs/heads/{branch}")).unwrap()
}

#[test]
fn branch_colours_graph_dark() {
    let (st, dir) = with_branches(ThemeChoice::Dark, "colour-graph-dark");
    let snap = st.snap.as_ref().unwrap();
    assert_ne!(snap.branch_swatch("topic"), snap.branch_swatch("fix-1"));
    render("branch_colours_graph_dark", st, true);
    remove_fixture(&dir);
}

#[test]
fn branch_colours_graph_light() {
    let (st, dir) = with_branches(ThemeChoice::Light, "colour-graph-light");
    render("branch_colours_graph_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn branch_colours_sidebar() {
    let (mut st, dir) = with_branches(ThemeChoice::Dark, "colour-sidebar");
    let workdir = st.snap.as_ref().unwrap().real_workdir.clone();
    let filter = GraphFilter { hidden: vec!["refs/heads/fix-1".into()], solo: vec![] };
    st.settings.set_graph_filter(&workdir, filter.clone());
    reload(&mut st, &dir, &LoadOptions { max_commits: 500, hidden: filter.hidden, ..Default::default() });
    render("branch_colours_sidebar", st, true);
    remove_fixture(&dir);
}

#[test]
fn branch_picker_menu_dots() {
    let (st, dir) = with_branches(ThemeChoice::Dark, "colour-picker");
    let mut h = harness(st, true);
    h.run();
    let picker = h.get_all_by_label("main").filter_map(|n| n.bounding_box()).find(|r| r.y1 < 100.0).expect("the Branch picker");
    click(&mut h, pos2(((picker.x0 + picker.x1) / 2.0) as f32, ((picker.y0 + picker.y1) / 2.0) as f32), PointerButton::Primary);
    assert_eq!(h.query_all_by_label("fix-1").count(), 2, "the sidebar's row and the menu's");
    let main = h.get_all_by_label("main").find(|n| n.role() == eframe::egui::accesskit::Role::RadioButton).expect("the menu's row");
    assert!(!main.is_disabled(), "the checked-out branch stays enabled");
    assert_eq!(main.toggled(), Some(eframe::egui::accesskit::Toggled::True), "and checked");
    h.check_snapshot("branch_picker_menu_dots");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_branch_row() {
    let (mut st, dir) = with_branches(ThemeChoice::Dark, "colour-details");
    let oid = tip(&dir, "topic");
    assert_eq!(st.snap.as_ref().unwrap().commit_branch(oid).map(|(k, _)| k), Some("topic"));
    st.session.selection = Selection::Commit(oid);
    st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let mut h = harness(st, true);
    h.run();
    let right = h.query_all_by_label("topic").filter_map(|n| n.bounding_box()).filter(|r| r.x0 > 900.0).count();
    assert_eq!(right, 1, "the details' branch row names its branch");
    h.check_snapshot("details_branch_row");
    drop(h);
    remove_fixture(&dir);
}

//! Stacked branches in the LOCAL section (indented under the branch they are built on, with
//! painted elbows and a "restack" part on rows that are behind) and the branch menu's stack
//! header note and restack items. The stacks are set by hand on the fixture's snapshot.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::stack::{RestackScope, StackLink, Stacks};
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The fixture with login-page stacked on search (`behind`: on an older tip of it) and an
/// unrelated branch, docs, off main.
pub(crate) fn with_stack(theme: ThemeChoice, name: &str, behind: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").cloned().unwrap();
    let mut docs = main.clone();
    (docs.name, docs.is_head) = ("docs".into(), false);
    snap.locals.insert(0, docs);
    let base = main.oid.unwrap();
    snap.stacks = Stacks { links: vec![StackLink { branch: "login-page".into(), on: "search".into(), base, behind }] };
    (st, dir)
}

/// Right-clicks the left panel's row labelled `label`.
pub(crate) fn row_menu(h: &mut H, label: &str) {
    let rect = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0).min_by(|a, b| a.y0.total_cmp(&b.y0));
    let rect = rect.expect("the sidebar row");
    click(h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
}

fn pushed(h: &H, want: &dyn Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn sidebar_stack() {
    let (st, dir) = with_stack(ThemeChoice::Dark, "sidebar-stack", false);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sidebar_stack");
    let y = |label: &str| h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0).map(|r| r.y0).next();
    assert!(y("login-page") > y("search"), "the stacked branch follows the branch it is on");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn sidebar_stack_restack_light() {
    let (st, dir) = with_stack(ThemeChoice::Light, "sidebar-stack-restack", true);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("sidebar_stack_restack_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_stacked() {
    let (st, dir) = with_stack(ThemeChoice::Dark, "branch-menu-stacked", true);
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "login-page");
    h.check_snapshot("branch_menu_stacked");
    assert!(h.query_by_label_contains("Stacked on search · needs restack").is_some());
    assert!(h.query_by_label("Restack branches above").is_none(), "nothing is stacked on login-page");
    let onto = h.get_by_label("Restack onto search");
    assert!(!onto.is_disabled());
    onto.click();
    h.run();
    let want = RestackScope::Onto("login-page".into());
    assert!(pushed(&h, &|m| matches!(m, Msg::Rebase(RebaseMsg::Restack { scope, .. }) if *scope == want)));

    row_menu(&mut h, "search");
    assert!(!h.get_by_label("Restack branches above").is_disabled());
    assert!(h.query_by_label_contains("Restack onto").is_none(), "search isn't stacked");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_stack_up_to_date() {
    let (st, dir) = with_stack(ThemeChoice::Light, "branch-menu-stack-current", false);
    let mut h = harness(st, false);
    h.run();
    row_menu(&mut h, "search");
    h.check_snapshot("branch_menu_stack_up_to_date");
    assert!(h.get_by_label("Restack branches above").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

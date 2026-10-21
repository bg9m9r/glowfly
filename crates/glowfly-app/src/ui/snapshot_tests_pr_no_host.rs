//! Pull request items in a repository with no remote on a code host: the LOCAL and REMOTE row
//! menus and a graph label's remote submenu keep one disabled item whose hover says why. Shares
//! the fixture repository and renderer with `snapshot_tests`.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::Msg;
use crate::state::{AppState, ForgeState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{Node, Queryable};
use git2::Oid;
use glowfly_core::forge::{Account, Provider, Target};
use glowfly_core::repo::{RefKind, RefLabel};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const ORIGIN_OFF_HOST: &str = "origin isn't on a code host such as GitHub or GitLab, so it can't take pull requests.";
const NO_REMOTES: &str = "Add a remote on a code host such as GitHub or GitLab to create pull requests.";

fn tip(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

/// The fixture with `origin` (a folder, so no code host) holding `main` and `search` at the
/// local tips, and the graph labels for them.
fn with_folder_remote(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let tips: Vec<(String, Oid)> = ["main", "search"].iter().map(|n| (n.to_string(), tip(&st, n))).collect();
    let snap = st.snap.as_mut().unwrap();
    for (n, oid) in &tips {
        let label =
            RefLabel { name: format!("origin/{n}"), full: format!("refs/remotes/origin/{n}"), kind: RefKind::Remote, is_head: false };
        snap.refs.entry(*oid).or_default().push(label);
    }
    snap.remotes = vec![("origin".into(), tips)];
    snap.remote_urls = vec![("origin".into(), "C:/remotes/project.git".into())];
    (st, dir)
}

/// Right-clicks the left panel's `search` row: the LOCAL one (topmost) or the REMOTE one, near
/// its left end.
fn open_row_menu(h: &mut H, local: bool) {
    let rows = h.get_all_by_label("search").filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0);
    let rect = if local { rows.min_by(|a, b| a.y0.total_cmp(&b.y0)) } else { rows.max_by(|a, b| a.y0.total_cmp(&b.y0)) };
    let rect = rect.expect("the sidebar row");
    click(h, egui::pos2(rect.x0 as f32 + 40.0, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
}

/// Rests the pointer on `label` (a disabled item) until its reason shows; whether `why` is on
/// screen.
pub(super) fn hover_shows(h: &mut H, label: &str, why: &str) -> bool {
    let at = center(&h.get_by_label(label));
    rest_on(h, at);
    h.query_by_label_contains(why).is_some()
}

fn center(n: &Node) -> egui::Pos2 {
    let r = n.bounding_box().expect("the item");
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

/// Glides the pointer onto `at` and lets its tooltip appear. egui shows one only once the
/// pointer has moved (at a speed, so over frames a few milliseconds apart, not the harness's
/// quarter-second steps) more recently than the click that opened the menu, and has then rested
/// for the tooltip delay. The frames carry their own times; the harness keeps the last one.
pub(super) fn rest_on(h: &mut H, at: egui::Pos2) {
    let t = h.ctx.input(|i| i.time);
    let frames = (0..4).map(|k| (0.01 * (k + 1) as f64, Some(at - egui::vec2((3 - k) as f32 * 2.0, 0.0))));
    let rest = (1..=4).map(|k| (0.04 + 0.25 * k as f64, None));
    for (dt, to) in frames.chain(rest) {
        let input = h.input_mut();
        input.time = Some(t + dt);
        input.events.extend(to.map(egui::Event::PointerMoved));
        h.step();
    }
    h.input_mut().time = None;
}

/// Clicks `label` and checks nothing was sent.
fn click_sends_nothing(h: &mut H, label: &str) {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    assert!(h.state().1.is_empty(), "a disabled item sends nothing");
}

#[test]
fn branch_menu_pr_no_host() {
    let (st, dir) = with_folder_remote(ThemeChoice::Dark, "branch-menu-pr-no-host");
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, true);
    let create = "Create pull request…";
    assert!(h.get_by_label(create).is_disabled());
    assert!(h.query_by_label("Push and create a pull request…").is_none(), "one item, not a group of dead ones");
    assert!(hover_shows(&mut h, create, ORIGIN_OFF_HOST));
    h.check_snapshot("branch_menu_pr_no_host");
    click_sends_nothing(&mut h, create);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_pr_no_remotes() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "branch-menu-pr-no-remotes");
    st.snap.as_mut().unwrap().remotes.clear();
    // An issue tracker alone is no code host either.
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    st.forge = Some(ForgeState::new(None, String::new(), Some(Target::jira(&jira, "PROJ"))));
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, true);
    assert!(h.get_by_label("Create pull request…").is_disabled());
    assert!(hover_shows(&mut h, "Create pull request…", NO_REMOTES));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_pr_no_host() {
    let (st, dir) = with_folder_remote(ThemeChoice::Light, "remote-branch-menu-pr-no-host");
    let mut h = harness(st, false);
    h.run();
    open_row_menu(&mut h, false);
    let create = "Create pull request from main into this branch…";
    assert!(h.get_by_label(create).is_disabled(), "main is checked out, search is another branch");
    assert!(h.query_by_label("Create pull request…").is_none(), "one item");
    assert!(hover_shows(&mut h, create, ORIGIN_OFF_HOST));
    click_sends_nothing(&mut h, create);
    drop(h);
    remove_fixture(&dir);
}

/// The graph label menu of main's tip: the "Remote origin/main" submenu, main being checked out.
#[test]
fn graph_label_remote_pr_no_host() {
    let (st, dir) = with_folder_remote(ThemeChoice::Dark, "label-remote-pr-no-host");
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(290.0, row_y(2)), PointerButton::Secondary);
    open_submenu(&mut h, "Remote origin/main");
    assert!(submenu_item(&h, "Remote origin/main", "Create pull request…").is_disabled(), "the checked-out branch itself");
    assert!(h.query_by_label("Create pull request from main into this branch…").is_none());
    let at = center(&submenu_item(&h, "Remote origin/main", "Create pull request…"));
    rest_on(&mut h, at);
    assert!(h.query_by_label_contains(ORIGIN_OFF_HOST).is_some());
    drop(h);
    remove_fixture(&dir);
}

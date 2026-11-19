//! Showing a pull request in the panel: the graph's PR chip (a click opens it without selecting
//! the row, a right-click keeps the row's menu) and the Show pull request item in the branch
//! menus, next to Open in the browser. Shares the fixtures of `snapshot_tests_pull_request` and
//! `snapshot_tests_push_pr`.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pr_fork::with_repos;
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_push_pr::{listed_pr, open_row_menu, tip, with_host, with_open_pr};
use super::snapshot_tests_select::click;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::AppState;

/// The graph with PR #12's chip on the `search` row and the panel closed.
fn with_chip(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, name, true);
    st.session.pull_request.view = None;
    let head = tip(&st, "search");
    st.session.forge.as_mut().unwrap().pr_heads.insert(12, head);
    (st, dir)
}

fn centre(h: &Harness<'_, (AppState, Vec<Msg>)>, label: &str) -> Pos2 {
    let r = h.get_by_label(label).bounding_box().expect("on screen");
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

/// The chip's centre: its button, not the tooltip's text of the same name.
fn chip_centre(h: &Harness<'_, (AppState, Vec<Msg>)>) -> Pos2 {
    let r = h.get_by_role_and_label(Role::Button, "Show pull request #12").bounding_box().expect("on screen");
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

fn opens(msgs: &[Msg], id: u64) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::Open(i)) if *i == id))
}

#[test]
fn graph_pr_chip_opens_the_panel() {
    let (st, dir) = with_chip("graph-pr-chip");
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_role_and_label(Role::Button, "Show pull request #12").is_some(), "the chip is a button");
    h.get_by_role_and_label(Role::Button, "Show pull request #12").hover();
    h.run_steps(4);
    h.check_snapshot("graph_pr_chip_tooltip");
    let at = chip_centre(&h);
    click(&mut h, at, PointerButton::Primary);
    let msgs = &h.state().1;
    assert!(opens(msgs, 12), "the chip opens its pull request");
    let selects = msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)) | Msg::Selection(_)));
    assert!(!selects, "the click doesn't select the row");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_pr_chip_keeps_the_row_menu() {
    let (st, dir) = with_chip("graph-pr-chip-menu");
    let mut h = harness(st, true);
    h.run_steps(4);
    let at = chip_centre(&h);
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.ctx.is_context_menu_open(), "the row's menu opens");
    assert!(h.query_by_label("Create patch…").is_some(), "the commit menu");
    assert!(!opens(&h.state().1, 12), "a right-click opens nothing");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_open_pr() {
    let (st, dir) = with_open_pr("branch-menu-open-pr", false);
    let mut h = harness(st, true);
    h.run_steps(4);
    open_row_menu(&mut h, true);
    h.check_snapshot("branch_menu_open_pr");
    let (show, browser) = (centre(&h, "Show pull request #7"), centre(&h, "Open pull request #7 in browser"));
    assert!(show.y < browser.y, "Show comes first");
    h.get_by_label("Show pull request #7").click();
    h.run();
    assert!(opens(&h.state().1, 7));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pr_menu_open_pr() {
    let (st, dir) = with_open_pr("pr-menu-open-pr", true);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More pull request actions").click();
    h.run();
    h.check_snapshot("pr_menu_open_pr");
    assert!(h.query_by_label("Show pull request #7").is_some());
    assert!(h.get_by_label("Push search and create a pull request…").is_disabled());
    h.get_by_label("Open pull request #7 in browser").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u.ends_with("/pull/7"))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_open_pr() {
    let (mut st, dir) = with_host(ThemeChoice::Light, "remote-branch-menu-open-pr", &["main", "search"]);
    st.session.forge.as_mut().unwrap().prs = vec![listed_pr(7, "search")];
    let mut h = harness(st, false);
    h.run();
    open_row_menu(&mut h, false);
    h.check_snapshot("remote_branch_menu_open_pr");
    assert!(h.query_by_label("Open pull request #7 in browser").is_some());
    h.get_by_label("Show pull request #7").click();
    h.run();
    assert!(opens(&h.state().1, 7));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_fork_open_pr() {
    let (mut st, dir) = with_host(ThemeChoice::Dark, "remote-branch-menu-fork-open-pr", &["main"]);
    with_repos(&mut st, true);
    let search = tip(&st, "search");
    st.snap.as_mut().unwrap().remotes[1].1.push(("search".into(), search));
    let mut pr = listed_pr(9, "search");
    pr.head_repo = glowfly_core::forge::HeadRepo::Fork { name: "ada/r".into() };
    st.session.forge.as_mut().unwrap().prs = vec![pr];
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, false);
    assert!(h.query_by_label("ada/search").is_some(), "the menu is the fork's branch");
    assert!(h.query_by_label("Open pull request #9 in browser").is_some());
    h.get_by_label("Show pull request #9").click();
    h.run();
    assert!(opens(&h.state().1, 9));
    drop(h);
    remove_fixture(&dir);
}

//! Pushing a branch and starting a pull request: the LOCAL and REMOTE row menus, the create
//! dialog's push note, and the toolbar's Push dropdown. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, Modal, Selection, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::forge::{Provider, PullRequest, Target};
use glowfly_core::repo::{BranchInfo, Tracking};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

pub(in crate::ui) fn tip(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

fn branch<'a>(st: &'a mut AppState, name: &str) -> &'a mut BranchInfo {
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == name).unwrap()
}

/// The fixture on GitHub through `origin`, whose loaded branches are `remote` (each at the
/// local branch's tip of the same name).
pub(in crate::ui) fn with_host(theme: ThemeChoice, name: &str, remote: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let mut f = ForgeState::new(Some(target), "origin".into(), None);
    f.host_status = Status::Ok;
    st.session.forge = Some(f);
    let branches = remote.iter().map(|n| (n.to_string(), tip(&st, n))).collect();
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), branches)];
    (st, dir)
}

/// `search` tracks origin/search and is `ahead` of it.
fn tracked(st: &mut AppState, ahead: usize) {
    let b = branch(st, "search");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "search".into() });
    b.upstream = Some("origin/search".into());
    b.upstream_ref = Some("refs/remotes/origin/search".into());
    b.ahead = ahead;
}

/// Checks out `search` (in the snapshot only).
fn check_out_search(st: &mut AppState) {
    branch(st, "main").is_head = false;
    branch(st, "search").is_head = true;
    st.snap.as_mut().unwrap().head_branch = Some("search".into());
}

/// Right-clicks the left panel's `search` row: the LOCAL one (topmost) or the REMOTE one.
pub(in crate::ui) fn open_row_menu(h: &mut Harness<'_, (AppState, Vec<Msg>)>, local: bool) {
    let rows = h.get_all_by_label("search").filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0);
    let rect = if local { rows.min_by(|a, b| a.y0.total_cmp(&b.y0)) } else { rows.max_by(|a, b| a.y0.total_cmp(&b.y0)) };
    let rect = rect.expect("the sidebar row");
    let center = egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(h, center, PointerButton::Secondary);
}

pub(in crate::ui) fn sent(h: &Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&ForgeMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Forge(f) if want(f)))
}

#[test]
fn branch_menu_push_pr() {
    let (mut st, dir) = with_host(ThemeChoice::Dark, "branch-menu-push-pr", &["main"]);
    st.session.selection = Selection::Commit(tip(&st, "main"));
    let mut h = harness(st, true);
    h.run_steps(4);
    open_row_menu(&mut h, true);
    h.check_snapshot("branch_menu_push_pr");
    assert!(h.query_by_label("Push and create a pull request…").is_some());
    assert!(h.query_by_label("Create pull request…").is_none(), "the service would refuse a missing source");
    h.get_by_label("Push and create a pull request into main…").click();
    h.run_steps(4);
    assert!(sent(&h, |f| matches!(f, ForgeMsg::PushAndStartPr { branch, target: Some(t) } if branch == "search" && t == "main")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_pr() {
    let (st, dir) = with_host(ThemeChoice::Light, "remote-branch-menu-pr", &["main", "search"]);
    let mut h = harness(st, false);
    h.run();
    open_row_menu(&mut h, false);
    h.check_snapshot("remote_branch_menu_pr");
    assert!(!h.get_by_label("Create pull request from main into this branch…").is_disabled(), "on a code host it is on");
    h.get_by_label("Create pull request…").click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::CreatePrDialog { source, target: None } if source == "search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_pr_dialog_push() {
    let (mut st, dir) = with_host(ThemeChoice::Dark, "create-pr-dialog-push", &["main", "search"]);
    tracked(&mut st, 2);
    st.modal = Some(Modal::CreatePr {
        source: "search".into(),
        target: "main".into(),
        targets: vec!["main".into()],
        title: "Search results page".into(),
        description: String::new(),
        draft: false,
        items: String::new(),
        repos: Default::default(),
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("create_pr_dialog_push");
    assert!(h.query_by_label("search has 2 commits that aren't on origin.").is_some());
    h.get_by_label("Push search").click();
    h.run();
    let push = |f: &ForgeMsg| matches!(f, ForgeMsg::PushToForPr { branch, remote, target: Some(t) } if branch == "search" && remote == "origin" && t == "main");
    assert!(sent(&h, push));
    assert!(h.query_by_label("Push to your fork…").is_none(), "only offered with the code host's repository known");
    assert!(matches!(h.state().0.modal, Some(Modal::CreatePr { .. })), "the dialog stays open");
    drop(h);
    remove_fixture(&dir);
}

/// With an issue tracker, the linked issues are a labelled optional field group, the typed
/// issues shown as chips under the field.
#[test]
fn create_pr_dialog_linked_issues() {
    let (mut st, dir) = with_host(ThemeChoice::Light, "create-pr-linked", &["main", "search"]);
    tracked(&mut st, 0);
    let tracker =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.session.forge.as_mut().unwrap().tracker = Some(tracker);
    st.modal = Some(Modal::CreatePr {
        source: "search".into(),
        target: "main".into(),
        targets: vec!["main".into()],
        title: "Search results page".into(),
        description: String::new(),
        draft: false,
        items: "12".into(),
        repos: Default::default(),
    });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("create_pr_dialog_linked_issues");
    assert!(h.query_all_by_label("Issues").any(|n| n.role() == egui::accesskit::Role::TextInput), "the label names the field");
    assert!(h.query_by_label("#12").is_some(), "the linked issue's chip");
    drop(h);
    remove_fixture(&dir);
}

/// In the description, Enter adds a line and Ctrl/Cmd+Enter creates the pull request.
#[test]
fn create_pr_dialog_ctrl_enter() {
    use eframe::egui::{Key, Modifiers};
    let (mut st, dir) = with_host(ThemeChoice::Dark, "create-pr-ctrl-enter", &["main", "search"]);
    tracked(&mut st, 0);
    st.modal = Some(Modal::CreatePr {
        source: "search".into(),
        target: "main".into(),
        targets: vec!["main".into()],
        title: "Search results page".into(),
        description: String::new(),
        draft: false,
        items: String::new(),
        repos: Default::default(),
    });
    let mut h = harness(st, true);
    h.run();
    let created = |h: &Harness<'_, (AppState, Vec<Msg>)>| sent(h, |f| matches!(f, ForgeMsg::CreatePr { .. }));
    // The commit box under the dialog has a Description too; the dialog's is drawn last.
    h.get_all_by_label("Description").last().expect("the dialog's description").focus();
    h.run();
    h.get_all_by_label("Description").last().expect("the dialog's description").type_text("Adds paging");
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(!created(&h), "Enter in the description adds a line");
    match &h.state().0.modal {
        Some(Modal::CreatePr { description, .. }) => assert_eq!(description, "Adds paging\n"),
        _ => panic!("the dialog stays open"),
    }
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(created(&h), "Ctrl/Cmd+Enter creates it");
    drop(h);
    remove_fixture(&dir);
}

/// Opens the toolbar's Push dropdown.
fn open_push_menu(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    h.get_by_label("More push actions").click();
    h.run();
}

#[test]
fn toolbar_push_menu_pr_item() {
    let label = "Push search and create a pull request…";
    let (mut st, dir) = with_host(ThemeChoice::Dark, "toolbar-push-pr", &["main"]);
    check_out_search(&mut st);
    let mut h = harness(st, true);
    h.run();
    open_push_menu(&mut h);
    assert!(!h.get_by_label(label).is_disabled(), "search is missing on origin");
    h.get_by_label(label).click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::PushAndStartPr { branch, target: None } if branch == "search")));
    drop(h);

    let (mut st, _) = with_host(ThemeChoice::Dark, "toolbar-push-pr", &["main", "search"]);
    check_out_search(&mut st);
    tracked(&mut st, 0);
    let mut h = harness(st, true);
    h.run();
    open_push_menu(&mut h);
    assert!(h.get_by_label(label).is_disabled(), "search is up to date on origin");
    drop(h);
    remove_fixture(&dir);
}
/// `search` tracks origin/topic (named differently), which has open pull request #7; `head`
/// checks it out.
fn with_open_pr(name: &str, head: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_host(ThemeChoice::Dark, name, &["main"]);
    let search = tip(&st, "search");
    st.snap.as_mut().unwrap().remotes[0].1.push(("topic".into(), search));
    let b = branch(&mut st, "search");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "topic".into() });
    b.upstream = Some("origin/topic".into());
    b.upstream_ref = Some("refs/remotes/origin/topic".into());
    let pr = PullRequest {
        id: 7,
        title: "Search results page".into(),
        description: String::new(),
        source: "topic".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: "https://github.com/o/r/pull/7".into(),
        facets: Default::default(),
    };
    st.session.forge.as_mut().unwrap().prs = vec![pr];
    if head {
        check_out_search(&mut st);
    }
    (st, dir)
}

#[test]
fn open_pr_under_another_upstream_name() {
    let (st, dir) = with_open_pr("open-pr-upstream-name", true);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More pull request actions").click();
    h.run();
    assert!(h.query_by_label("Open pull request #7").is_some());
    assert!(h.query_by_label("Create pull request…").is_none(), "search already has #7");
    assert!(h.get_by_label("Push search and create a pull request…").is_disabled());
    drop(h);

    let (st, _) = with_open_pr("open-pr-upstream-name", false);
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, true);
    assert!(h.query_by_label("Open pull request #7").is_some());
    assert!(h.query_by_label("Create pull request…").is_none());
    h.get_by_label("Open pull request #7").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u.ends_with("/pull/7"))));
    drop(h);
    remove_fixture(&dir);
}

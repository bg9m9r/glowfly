//! View on service and the fork dialogs: the remote row menu for a GitHub remote, the Add a fork
//! dialog (a list with a private fork and one already added, and no forks at all), the Fork
//! dialog (a fork the user already has, and the confirmation) and the Add remote dialog's fork
//! link. Remotes are injected into the shared fixture's snapshot with fixed URLs.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, center, dialog_button, pushed};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::forks::{ForkCreate, ForkPicker, ForkStep, ForkThenPr};
use crate::state::remotes::RemoteForm;
use crate::state::{AppState, Modal};
use crate::ui::remotes::fork_pick::PICK_FIRST;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Fork, ForkList, Target, parse_remote};
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The fixture with `origin` on GitHub (two branches), `backup` in a folder and `linus` (a
/// teammate's fork, no branches yet).
fn with_github(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    snap.remotes = vec![
        ("origin".into(), vec![("main".into(), main), ("search".into(), search)]),
        ("backup".into(), Vec::new()),
        ("linus".into(), Vec::new()),
    ];
    let remote =
        |name: &str, url: &str| RemoteInfo { name: name.into(), urls: vec![url.into()], push_urls: Vec::new(), skip_fetch_all: false };
    snap.remote_info = vec![
        remote("origin", "https://github.com/team/app.git"),
        remote("backup", "D:/backups/app.git"),
        remote("linus", "git@github.com:linus/app.git"),
    ];
    (st, dir)
}

fn target() -> Target {
    parse_remote("https://github.com/team/app.git", &[]).unwrap()
}

fn fork(owner: &str, private: bool) -> Fork {
    Fork {
        owner: owner.into(),
        full_name: format!("{owner}/app"),
        https_url: format!("https://github.com/{owner}/app.git"),
        ssh_url: format!("git@github.com:{owner}/app.git"),
        web_url: format!("https://github.com/{owner}/app"),
        private,
    }
}

fn picker(st: &AppState, forks: Vec<Fork>) -> ForkPicker {
    ForkPicker {
        workdir: st.snap.as_ref().unwrap().workdir.clone(),
        remote: "origin".into(),
        target: target(),
        loading: false,
        list: ForkList { forks, truncated: false },
        error: None,
        filter: String::new(),
        picked: None,
        name: String::new(),
        suggested: String::new(),
        fetch_after: true,
        touched: Default::default(),
    }
}

fn create(st: &AppState, step: ForkStep, name: &str) -> ForkCreate {
    ForkCreate {
        workdir: st.snap.as_ref().unwrap().workdir.clone(),
        remote: "origin".into(),
        target: target(),
        step,
        name: name.into(),
        fetch_after: true,
        error: None,
        touched: Default::default(),
        then_pr: None,
    }
}

fn button_disabled(h: &H, label: &str) -> bool {
    dialog_button(h, label).is_disabled()
}

#[test]
fn remote_group_menu_github() {
    let (st, dir) = with_github(ThemeChoice::Dark, "remote-group-menu-github");
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "origin");
    click(&mut h, at, PointerButton::Secondary);
    for item in ["View on GitHub", "Add a fork…", "Fork on GitHub…"] {
        assert!(!h.get_by_label(item).is_disabled(), "{item}");
    }
    h.check_snapshot("remote_group_menu_github");
    h.get_by_label("View on GitHub").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::View(n)) if n == "origin")));

    let at = center(&h, "backup");
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("View in browser").is_disabled(), "a folder has no web page");
    assert!(h.query_by_label("Add a fork…").is_none(), "a folder is on no hosting service: no fork items");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn fork_picker_dialog() {
    let (mut st, dir) = with_github(ThemeChoice::Dark, "fork-picker-dialog");
    let mut p = picker(&st, vec![fork("ada", false), fork("grace", true), fork("linus", false)]);
    p.picked = Some(0);
    p.name = "ada".into();
    p.suggested = "ada".into();
    st.modal = Some(Modal::AddFork(Box::new(p)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("added as linus").is_some());
    assert!(h.query_by_label("private").is_some());
    assert!(!button_disabled(&h, "Add remote"));
    h.check_snapshot("fork_picker_dialog");
    h.get_by_label("grace/app").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::PickFork(1)))));
    dialog_button(&h, "Add remote").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddFork))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn fork_picker_says_pick_first() {
    let (mut st, dir) = with_github(ThemeChoice::Dark, "fork-picker-pick-first");
    st.modal = Some(Modal::AddFork(Box::new(picker(&st, vec![fork("ada", false), fork("grace", true)]))));
    let mut h = harness(st, true);
    h.run();
    assert!(button_disabled(&h, "Add remote"));
    assert!(h.query_by_label(PICK_FIRST).is_some(), "the footer says why before a pick");
    if let Some(Modal::AddFork(p)) = h.state_mut().0.modal.as_mut() {
        p.picked = Some(0);
        p.name = "ada".into();
        p.suggested = "ada".into();
    }
    h.run();
    assert!(!button_disabled(&h, "Add remote"));
    assert!(h.query_by_label(PICK_FIRST).is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn fork_picker_empty_light() {
    let (mut st, dir) = with_github(ThemeChoice::Light, "fork-picker-empty");
    st.modal = Some(Modal::AddFork(Box::new(picker(&st, Vec::new()))));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Nobody has forked team/app yet.").is_some());
    h.check_snapshot("fork_picker_empty_light");
    h.get_by_label("Fork it on GitHub…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::ForkOnService(n)) if n == "origin")));

    // Loading: a spinner, which keeps repainting.
    if let Some(Modal::AddFork(p)) = h.state_mut().0.modal.as_mut() {
        p.loading = true;
    }
    h.run_steps(3);
    assert!(h.query_by_label("Loading forks…").is_some());
    assert_eq!(h.query_all_by_role_and_label(Role::Button, "Add remote").count(), 1, "only the REMOTE header's +");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn fork_create_found() {
    let (mut st, dir) = with_github(ThemeChoice::Dark, "fork-create-found");
    let step = ForkStep::Mine { user: "ada".into(), fork: fork("ada", false) };
    st.modal = Some(Modal::CreateFork(Box::new(create(&st, step, "ada"))));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("You already have a fork of team/app: ada/app.").is_some());
    assert!(!button_disabled(&h, "Add as remote"));
    h.check_snapshot("fork_create_found");
    h.get_by_role_and_label(Role::Button, "Add as remote").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddMyFork))));
    drop(h);
    remove_fixture(&dir);
}

/// Opened from the create dialog's Push to your fork…: the fork is also pushed to.
#[test]
fn fork_dialog_then_push() {
    let (mut st, dir) = with_github(ThemeChoice::Dark, "fork-dialog-then-push");
    let step = ForkStep::Mine { user: "ada".into(), fork: fork("ada", false) };
    let mut f = create(&st, step, "ada");
    f.then_pr = Some(ForkThenPr { branch: "search".into(), target: None });
    st.modal = Some(Modal::CreateFork(Box::new(f)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Then pushes search to the fork and opens the Create pull request dialog.").is_some());
    h.check_snapshot("fork_dialog_then_push");
    h.get_by_role_and_label(Role::Button, "Add as remote and push").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddMyFork))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn fork_create_ready_light() {
    let (mut st, dir) = with_github(ThemeChoice::Light, "fork-create-ready");
    st.modal = Some(Modal::CreateFork(Box::new(create(&st, ForkStep::Ready { user: "ada".into() }, "ada"))));
    let mut h = harness(st, false);
    h.run();
    let text = "Creates a copy of team/app under your account (ada) on GitHub, adds it as a remote and fetches it.";
    assert!(h.query_by_label(text).is_some());
    h.check_snapshot("fork_create_ready_light");
    h.get_by_role_and_label(Role::Button, "Fork and add remote").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::CreateFork))));

    if let Some(Modal::CreateFork(f)) = h.state_mut().0.modal.as_mut() {
        f.step = ForkStep::Creating { user: "ada".into() };
    }
    h.run_steps(3);
    assert!(h.query_by_label("Creating the fork on GitHub… This can take a few seconds.").is_some());
    assert!(button_disabled(&h, "Fork and add remote"));
    assert!(!button_disabled(&h, "Cancel"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_add_dialog_fork_link() {
    let (mut st, dir) = with_github(ThemeChoice::Dark, "remote-add-dialog-fork-link");
    st.modal = Some(Modal::Remote(RemoteForm::add(st.snap.as_ref().unwrap())));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("remote_add_dialog_fork_link");
    h.get_by_label("Pick a fork of origin on GitHub…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::ForkDialog(n)) if n == "origin")));
    drop(h);
    remove_fixture(&dir);
}

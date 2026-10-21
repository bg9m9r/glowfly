//! Pushing a stack and creating its pull requests: the branch menu's Push stack and Create pull
//! requests for the stack items (enabled, and Push stack disabled once every layer is on
//! origin), and the create dialog as a step of a stack's run (its step line and Skip).

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pr_no_host::hover_shows;
use super::snapshot_tests_push_pr::with_host;
use super::snapshot_tests_stack::row_menu;
use crate::msg::{Msg, StackMsg};
use crate::state::stack_prs::{PrLayer, StackPrRun, StackPrs};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::stack::{StackLink, Stacks};

/// The fixture on GitHub with `remote` on origin, and login-page stacked on search.
fn stacked(theme: ThemeChoice, name: &str, remote: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_host(theme, name, remote);
    let snap = st.snap.as_mut().unwrap();
    let base = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    snap.stacks = Stacks { links: vec![StackLink { branch: "login-page".into(), on: "search".into(), base, behind: false }] };
    (st, dir)
}

fn sent(st: &(AppState, Vec<Msg>), want: impl Fn(&StackMsg) -> bool) -> bool {
    st.1.iter().any(|m| matches!(m, Msg::Stack(s) if want(s)))
}

#[test]
fn branch_menu_stack_push() {
    let (st, dir) = stacked(ThemeChoice::Dark, "branch-menu-stack-push", &["main"]);
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "login-page");
    h.check_snapshot("branch_menu_stack_push");
    let push = h.get_by_label("Push stack");
    assert!(!push.is_disabled());
    push.click();
    h.run();
    assert!(sent(h.state(), |m| matches!(m, StackMsg::Push { branch } if branch == "login-page")));
    row_menu(&mut h, "search");
    let create = h.get_by_label("Create pull requests for the stack…");
    assert!(!create.is_disabled());
    create.click();
    h.run();
    assert!(sent(h.state(), |m| matches!(m, StackMsg::CreatePrs { branch } if branch == "search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_stack_pushed_light() {
    let (st, dir) = stacked(ThemeChoice::Light, "branch-menu-stack-pushed", &["main", "search", "login-page"]);
    let mut h = harness(st, false);
    h.run();
    row_menu(&mut h, "login-page");
    assert!(h.get_by_label("Push stack").is_disabled());
    assert!(hover_shows(&mut h, "Push stack", "Every branch in the stack is already pushed."));
    assert!(!h.get_by_label("Create pull requests for the stack…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_pr_stack_step() {
    let (mut st, dir) = stacked(ThemeChoice::Dark, "create-pr-stack-step", &["main", "search", "login-page"]);
    let layer = PrLayer { branch: "login-page".into(), source: "login-page".into(), target: Some("search".into()) };
    let run = StackPrRun { current: Some(layer), index: 2, total: 2, created: vec!["search".into()], ..Default::default() };
    st.forge.as_mut().unwrap().stack_prs = Some(StackPrs::Running(run));
    st.modal = Some(Modal::CreatePr {
        source: "login-page".into(),
        target: "search".into(),
        targets: vec!["main".into(), "search".into()],
        title: "Login page".into(),
        description: String::new(),
        draft: false,
        items: String::new(),
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("create_pr_stack_step");
    assert!(h.query_by_label("Branch 2 of 2 in the stack. Its base is the branch below it.").is_some());
    h.get_by_label("Skip").click();
    h.run();
    assert!(sent(h.state(), |m| matches!(m, StackMsg::SkipPr)));
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    h.state_mut().1.clear();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(sent(h.state(), |m| matches!(m, StackMsg::StopPrs)));
    drop(h);
    remove_fixture(&dir);
}

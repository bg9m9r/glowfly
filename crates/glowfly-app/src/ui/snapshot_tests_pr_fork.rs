//! Pull requests across forks: the create dialog's From / Into rows with two PR repositories,
//! its Push to your fork… with one, and the REMOTE row menu of a fork remote. Shares the push
//! PR fixture (`with_host`).

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_push_pr::{open_row_menu, sent, tip, with_host};
use crate::msg::ForgeMsg;
use crate::state::pr_repos::{PrRepo, PrRepoPick, RepoSide};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Target, parse_remote};
use glowfly_core::settings::ThemeChoice;

fn t(url: &str) -> Target {
    parse_remote(url, &[]).unwrap()
}

/// Lists `origin` (o/r) and, with `fork`, `ada` (ada/r, with only `main`) as PR repositories.
fn with_repos(st: &mut AppState, fork: bool) {
    let mut repos = vec![PrRepo { remote: "origin".into(), target: t("https://github.com/o/r") }];
    if fork {
        repos.push(PrRepo { remote: "ada".into(), target: t("https://github.com/ada/r") });
        let main = tip(st, "main");
        st.snap.as_mut().unwrap().remotes.push(("ada".into(), vec![("main".into(), main)]));
    }
    st.forge.as_mut().unwrap().pr_repos = repos;
}

fn dialog(st: &mut AppState, from: &str) {
    st.modal = Some(Modal::CreatePr {
        source: "search".into(),
        target: "main".into(),
        targets: vec!["main".into()],
        title: "Search results page".into(),
        description: String::new(),
        draft: false,
        items: String::new(),
        repos: PrRepoPick { branch: "search".into(), from: from.into(), into: "origin".into() },
    });
}

#[test]
fn create_pr_dialog_repositories() {
    let (mut st, dir) = with_host(ThemeChoice::Dark, "create-pr-dialog-repositories", &["main"]);
    with_repos(&mut st, true);
    dialog(&mut st, "ada");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("create_pr_dialog_repositories");
    assert!(h.query_by_label("search isn't on ada yet.").is_some());
    assert!(h.query_by_label("Push to your fork…").is_none(), "the fork is a remote already");
    h.get_by_label("Push search").click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::PushToForPr { branch, remote, .. } if branch == "search" && remote == "ada")));
    // The From picker: the dialog's topmost combo box.
    let in_dialog = |n: &egui_kittest::kittest::Node<'_>| n.bounding_box().is_some_and(|r| r.x0 > 380.0);
    let combos = h.query_all_by_role(Role::ComboBox).filter(in_dialog);
    let from = combos.min_by(|a, b| a.bounding_box().unwrap().y0.total_cmp(&b.bounding_box().unwrap().y0));
    from.expect("the From picker").click();
    h.run();
    assert!(h.query_by_label("ada/r (ada)").is_some(), "both repositories are listed");
    h.get_by_label("o/r (origin)").click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::PickPrRepo { side: RepoSide::From, remote } if remote == "origin")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_pr_dialog_push_to_fork() {
    let (mut st, dir) = with_host(ThemeChoice::Light, "create-pr-dialog-push-to-fork", &["main"]);
    with_repos(&mut st, false);
    dialog(&mut st, "origin");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("create_pr_dialog_push_to_fork");
    assert!(h.query_by_label("search isn't on origin yet.").is_some());
    assert!(h.query_by_label("From").is_none(), "one repository: no From / Into rows");
    h.get_by_label("Push to your fork…").click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::PushToMyFork { branch, .. } if branch == "search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_fork_pr() {
    let (mut st, dir) = with_host(ThemeChoice::Dark, "remote-branch-menu-fork-pr", &["main"]);
    with_repos(&mut st, true);
    let search = tip(&st, "search");
    st.snap.as_mut().unwrap().remotes[1].1.push(("search".into(), search));
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, false);
    assert!(h.query_by_label("ada/search").is_some(), "the menu is the fork's branch");
    h.get_by_label("Create pull request…").click();
    h.run();
    assert!(sent(&h, |f| matches!(f, ForgeMsg::CreatePrFrom { remote, source } if remote == "ada" && source == "search")));
    drop(h);
    remove_fixture(&dir);
}

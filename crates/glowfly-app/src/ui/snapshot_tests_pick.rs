//! Cherry-pick and revert views: the mainline parent dialog for a merge commit, and the WIP
//! banner of a stopped cherry-pick (Skip commit on, and off for an old git). Shares the fixture
//! repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, try_git, with_repo};
use crate::state::pick::{MainlineForm, PickOp};
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::git_version::GitInfo;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

#[test]
fn mainline_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "mainline-dialog");
    let snap = st.snap.as_ref().unwrap();
    let merge = snap.commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    let form = MainlineForm::new(PickOp::CherryPick, &[merge], snap).expect("a merge");
    assert_eq!(form.merges[0].parents.len(), 2);
    st.modal = Some(Modal::Mainline(form));
    render("mainline_dialog", st, true);
    remove_fixture(&dir);
}

/// Clicking a parent takes the focus off the default button (a radio doesn't keep it); Enter
/// still runs the pick, with the parent chosen.
#[test]
fn mainline_radio_then_enter() {
    use crate::msg::{Msg, SelectionMsg};
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "mainline-enter");
    let snap = st.snap.as_ref().unwrap();
    let merge = snap.commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    st.modal = Some(Modal::Mainline(MainlineForm::new(PickOp::CherryPick, &[merge], snap).expect("a merge")));
    let mut h = harness(st, true);
    h.run();
    h.get_by_label_contains("Parent 2:").click();
    h.run();
    h.press_key(eframe::egui::Key::Enter);
    h.run();
    let picked = |m: &Msg| matches!(m, Msg::Selection(SelectionMsg::PickWithMainline { mainline: 2, .. }));
    assert!(h.state().1.iter().any(picked), "Enter picks parent 2");
    drop(h);
    remove_fixture(&dir);
}

/// The fixture stopped in a cherry-pick: "Fix build on Windows" picked onto a branch from "Add
/// app skeleton" that changed app.txt too.
pub(super) fn stopped_pick(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let d = "2026-01-11T10:00:00+00:00";
    let snap = st.snap.as_ref().unwrap();
    let oid = |s: &str| snap.commits.iter().find(|c| c.summary == s).unwrap().oid.to_string();
    let (skeleton, fix) = (oid("Add app skeleton"), oid("Fix build on Windows"));
    git(&dir, &["checkout", "-q", "-b", "hotfix", &skeleton], d);
    std::fs::write(dir.join("app.txt"), "app\nhotfix\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "Hotfix the app"], d);
    let (ok, _) = try_git(&dir, &["cherry-pick", &fix], d);
    assert!(!ok, "the pick conflicts");
    reload(&mut st, &dir, &LoadOptions::all(500));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::CherryPick);
    st.selection = Selection::Wip;
    (st, dir)
}

#[test]
fn stopped_cherry_pick_banner() {
    let (st, dir) = stopped_pick("pick-banner");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("cherry_pick_banner");
    h.get_by_label("More ways to go on").click();
    h.run();
    assert!(!h.get_by_label("Skip commit").is_disabled());
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = stopped_pick("pick-banner-old-git");
    st.git.info = Some(GitInfo { path: None, version: Some((2, 22, 0)), chosen: false });
    st.git.banner_dismissed = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("cherry_pick_banner_old_git");
    h.get_by_label("More ways to go on").click();
    h.run();
    assert!(h.get_by_label("Skip commit").is_disabled(), "too old a git: a disabled item with its reason");
    drop(h);
    remove_fixture(&dir);
}

//! Git Flow snapshots: Preferences > Git Flow not set up yet, set up (with the Start buttons and
//! the Finish row), edited with overlapping prefixes, the Start dialog for a release, the Finish
//! dialog for a release and for a feature blocked by uncommitted changes; the left panel's GIT
//! FLOW section (also filtered, and its header menu), the Git Flow panel (also not set up) and
//! the issue menu's Start Git Flow items.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow::{FinishForm, StartForm};
use crate::state::{AppState, ForgeState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Issue, Provider, Target};
use glowfly_core::gitflow::FlowKind;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// Opens the Git Flow page with its form filled from the snapshot.
fn open_page(st: &mut AppState) {
    let mut prefs = PrefsState::new(PrefPage::RepoGitFlow);
    prefs.gitflow.fill(st.snap.as_ref().unwrap());
    st.prefs = Some(prefs);
}

/// The fixture with a develop branch and the gitflow config (version tag prefix v).
pub(crate) fn initialized(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["branch", "develop"], d);
    for (key, value) in [
        ("gitflow.branch.master", "main"),
        ("gitflow.branch.develop", "develop"),
        ("gitflow.prefix.feature", "feature/"),
        ("gitflow.prefix.release", "release/"),
        ("gitflow.prefix.hotfix", "hotfix/"),
        ("gitflow.prefix.versiontag", "v"),
    ] {
        git(&dir, &["config", key, value], d);
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

#[test]
fn prefs_gitflow_setup() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "gitflow-setup");
    open_page(&mut st);
    render("prefs_gitflow_setup", st, false);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-set-up");
    open_page(&mut st);
    render("prefs_gitflow", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow_edited() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-edited");
    open_page(&mut st);
    let form = &mut st.prefs.as_mut().unwrap().gitflow.form;
    form.feature = "feat/".into();
    form.release = "feat/".into();
    render("prefs_gitflow_edited", st, true);
    remove_fixture(&dir);
}

#[test]
fn gitflow_start_release() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-start");
    st.modal = Some(Modal::GitFlowStart(StartForm { kind: FlowKind::Release, name: "0.2.0".into(), touched: Default::default() }));
    render("gitflow_start_release", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow_finish() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-finish-row");
    git(&dir, &["branch", "release/0.2.0"], "2026-01-05T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    open_page(&mut st);
    render("prefs_gitflow_finish", st, true);
    remove_fixture(&dir);
}

#[test]
fn gitflow_finish_release() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-finish-release");
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["checkout", "--", "README.md"], d);
    git(&dir, &["branch", "release/0.2.0"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.modal = Some(Modal::GitFlowFinish(FinishForm {
        kind: FlowKind::Release,
        branch: "release/0.2.0".into(),
        tag: "v0.2.0".into(),
        message: "Release 0.2.0".into(),
        keep: false,
        focus_tag: true,
        touched: Default::default(),
    }));
    render("gitflow_finish_release", st, true);
    remove_fixture(&dir);
}

/// The fixture's work tree has uncommitted changes: Finish offers to stash them first.
#[test]
fn gitflow_finish_feature_stash() {
    let (mut st, dir) = initialized(ThemeChoice::Light, "gitflow-finish-stash");
    git(&dir, &["branch", "feature/search", "search"], "2026-01-05T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.modal = Some(Modal::GitFlowFinish(FinishForm {
        kind: FlowKind::Feature,
        branch: "feature/search".into(),
        tag: String::new(),
        message: String::new(),
        keep: false,
        focus_tag: false,
        touched: Default::default(),
    }));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("gitflow_finish_feature_stash");
    assert!(h.query_by_label_contains("stashed first and put back on develop once Finish is done").is_some());
    h.get_by_label("Stash changes and finish").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::StashAndFinish { branch, .. }) if branch == "feature/search")));
    remove_fixture(&dir);
}

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The initialized fixture with feature/login (checked out), feature/search and release/1.0.0.
pub(crate) fn with_flow_branches(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = initialized(theme, name);
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["branch", "feature/login"], d);
    git(&dir, &["branch", "feature/search", "search"], d);
    git(&dir, &["branch", "release/1.0.0"], d);
    git(&dir, &["checkout", "-q", "feature/login"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

pub(crate) fn center(h: &H, label: &str) -> Pos2 {
    let rect = h.get_by_label(label).bounding_box().expect("the row");
    pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32)
}

pub(crate) fn pushed(h: &H, want: &dyn Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn gitflow_section() {
    let (st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-section");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_section");
    h.get_by_label("Start hotfix").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::StartDialog(FlowKind::Hotfix)))));
    h.get_by_label("1.0.0").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Repo(crate::msg::RepoMsg::JumpTo(_)))));

    // The row menu: Finish first, then the local branch actions.
    let at = center(&h, "1.0.0");
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.query_by_label("Finish release 1.0.0…").is_some());
    assert!(h.query_by_label("Checkout").is_some());
    h.get_by_label("Finish release 1.0.0…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::FinishDialog(b)) if b == "release/1.0.0")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_section_header_menu() {
    let (st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-header-menu");
    let mut h = harness(st, true);
    h.run();
    // The header sits right above the Features group row (11 + 15 px between their centers).
    let row = center(&h, "Features");
    click(&mut h, pos2(row.x, row.y - 26.0), PointerButton::Secondary);
    h.check_snapshot("gitflow_section_header_menu");
    for label in ["Start feature…", "Start release…", "Start hotfix…", "Configure Git Flow…"] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
    h.get_by_label("Open Git Flow panel").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::OpenPanel))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_section_filtered() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-section-filtered");
    st.session.tab_view.ref_filter = "log".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_section_filtered");
    assert!(h.query_by_label("Features").is_some());
    assert!(h.query_by_label("Releases").is_none() && h.query_by_label("Hotfixes").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_panel() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-panel");
    st.session.gitflow_panel = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_panel");
    h.get_by_label("Start hotfix…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::StartDialog(FlowKind::Hotfix)))));
    h.get_by_label("Close").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::ClosePanel))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_panel_not_set_up() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "gitflow-panel-off");
    st.session.gitflow_panel = true;
    render("gitflow_panel_not_set_up", st, false);
    remove_fixture(&dir);
}

#[test]
fn gitflow_issue_menu() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-issue-menu");
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let mut f = ForgeState::new(Some(target.clone()), "origin".into(), Some(target));
    f.my_issues = vec![Issue {
        key: "12".into(),
        title: "Crash on start".into(),
        state: "open".into(),
        kind: "Issue".into(),
        assignee: Some("ada".into()),
        url: "https://github.com/o/r/issues/12".into(),
        facets: Default::default(),
    }];
    st.session.forge = Some(f);
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "#12  Crash on start");
    click(&mut h, at, PointerButton::Secondary);
    h.check_snapshot("gitflow_issue_menu");
    assert!(h.query_by_label("Create branch from issue…").is_some());
    h.get_by_label("Start Git Flow release from issue…").click();
    h.run();
    let want =
        |m: &Msg| matches!(m, Msg::GitFlow(GitFlowMsg::StartFromIssue { kind: FlowKind::Release, name }) if name == "12-crash-on-start");
    assert!(pushed(&h, &want));
    drop(h);
    remove_fixture(&dir);
}

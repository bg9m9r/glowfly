//! The pull request panel's Checks section: a list with every state, the empty state (on GitHub
//! and beside the conversation note of a service without in-app reviews), the section closed,
//! and a failed load; and the messages its header, rows and Try again send.

use super::snapshot_tests::{NOW, harness, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_pull_request_review::{detail, load};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::PrView;
use crate::state::pull_request_checks::{ChecksPane, HELD, LOADING};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{CheckList, CheckRun, CheckState, Checks, Provider, RunState, Target};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

const BUILD_URL: &str = "https://ci.example/acme/demo/build/41";

fn check(name: &str, group: Option<&str>, state: RunState) -> CheckRun {
    CheckRun {
        name: name.into(),
        group: group.map(str::to_owned),
        state,
        required: None,
        started: None,
        finished: None,
        url: format!("https://ci.example/acme/demo/{name}"),
        description: String::new(),
    }
}

/// One check of every state, in list order.
fn every_state() -> CheckList {
    let mut build = check("build", Some("CI"), RunState::Failed);
    (build.required, build.started, build.finished) = (Some(true), Some(NOW - 3600), Some(NOW - 3600 + 134));
    build.url = BUILD_URL.into();
    let mut test = check("test", Some("CI"), RunState::Running);
    (test.required, test.started) = (Some(true), Some(NOW - 200));
    let mut docs = check("docs", Some("Pages"), RunState::Passed);
    (docs.started, docs.finished) = (Some(NOW - 7200), Some(NOW - 7200 + 45));
    let mut coverage = check("coverage", None, RunState::Neutral);
    coverage.url = String::new();
    let runs = vec![
        build,
        check("approve-deploy", Some("Release"), RunState::ActionRequired),
        test,
        check("e2e", Some("CI"), RunState::Pending),
        check("lint", Some("CI"), RunState::Cancelled),
        docs,
        coverage,
        check("windows", Some("CI"), RunState::Skipped),
    ];
    CheckList { sha: String::new(), runs, complete: true }
}

/// `with_pr` on the Overview tab with its page loaded, the checks set by `f`.
fn with_checks(theme: ThemeChoice, name: &str, f: impl FnOnce(&mut PrView)) -> (AppState, PathBuf) {
    let (mut st, dir) = with_pr(theme, name, true);
    load(&mut st, detail(), Vec::new());
    f(st.pull_request.as_mut().unwrap());
    (st, dir)
}

fn sent(h: &egui_kittest::Harness<'static, (AppState, Vec<Msg>)>, f: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(f)
}

#[test]
fn pr_check_list_dark() {
    let (mut st, dir) = with_checks(ThemeChoice::Dark, "pr-check-list", |v| {
        v.checks.pane = ChecksPane::Loaded(every_state());
        v.pr.checks = Some(Checks { url: BUILD_URL.into(), ..Checks::from_counts(1, 1, 2, true) });
    });
    let listed = st.pull_request.as_ref().unwrap().pr.checks.clone();
    st.forge.as_mut().unwrap().prs[0].checks = listed;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pr_check_list_dark");
    assert!(h.query_by_label("approve-deploy: Needs action").is_some() && h.query_by_label("e2e: Queued").is_some());
    h.get_by_label("Open failed build").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::OpenUrl(u) if u == BUILD_URL)));
    h.state_mut().1.clear();
    h.get_by_label("lint: Cancelled").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::OpenUrl(u) if u == "https://ci.example/acme/demo/lint")));
    h.state_mut().1.clear();
    h.get_by_label("build: Failed").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::OpenUrl(u) if u == BUILD_URL)));
    // Without a link a click does nothing.
    h.state_mut().1.clear();
    h.get_by_label("coverage: Neutral").click();
    h.run();
    assert!(h.state().1.is_empty());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pr_check_list_empty_light() {
    let (st, dir) = with_checks(ThemeChoice::Light, "pr-check-empty", |v| v.checks.pane = ChecksPane::Loaded(CheckList::empty("")));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pr_check_list_empty_light");
    assert!(h.query_by_label("No build status").is_some());
    assert!(h.query_by_label("Nothing has reported a check or build on this pull request's latest commit yet.").is_some());
    assert!(h.query_by_label_contains("Open build page").is_none());
    drop(h);

    // A service without in-app reviews (Azure DevOps) still shows its checks.
    let (mut st, _) = with_pr(ThemeChoice::Light, "pr-check-empty", true);
    let fs = st.forge.as_mut().unwrap();
    fs.host = Some(Target {
        provider: Provider::AzureDevOps,
        host: "dev.azure.com".into(),
        owner: "acme".into(),
        project: "web".into(),
        repo: "demo".into(),
    });
    let mut view = PrView::new(fs.prs[0].clone(), fs, st.snap.as_ref().unwrap());
    view.checks.pane = ChecksPane::Loaded(CheckList::empty(""));
    st.pull_request = Some(view);
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label_contains("work with GitHub, GitLab and Bitbucket for now").is_some());
    assert!(h.query_by_label("Nothing has reported a check or build on this pull request's latest commit yet.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pr_check_list_collapsed_light() {
    let (st, dir) = with_checks(ThemeChoice::Light, "pr-check-closed", |v| {
        v.checks.pane = ChecksPane::Loaded(every_state());
        v.checks.open = false;
    });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pr_check_list_collapsed_light");
    assert!(h.query_by_label("build: Failed").is_none(), "no rows");
    assert!(h.query_by_label("Open failed build").is_none());
    h.get_by_role_and_label(Role::Button, "Checks").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::PullRequest(PullRequestMsg::ToggleChecks))));
    drop(h);
    assert_eq!(every_state().overall(), Some(CheckState::Failed), "the header's marker");
    remove_fixture(&dir);
}

#[test]
fn pr_check_list_failed_dark() {
    let (st, dir) = with_checks(ThemeChoice::Dark, "pr-check-failed", |v| {
        v.checks.pane = ChecksPane::Failed("HTTP 502: bad gateway".into());
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pr_check_list_failed_dark");
    assert!(h.query_by_label("Couldn't load the checks").is_some());
    h.get_by_label("Try again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::PullRequest(PullRequestMsg::ReloadChecks))));
    drop(h);

    // Loading (a spinner keeps repainting, so only a few frames are drawn) and held.
    let (st, _) = with_checks(ThemeChoice::Dark, "pr-check-failed", |v| v.checks.pane = ChecksPane::Loading);
    let mut h = harness(st, true);
    h.run_steps(3);
    assert!(h.query_by_label(LOADING).is_some());
    drop(h);
    let (st, _) = with_checks(ThemeChoice::Dark, "pr-check-failed", |v| v.checks.pane = ChecksPane::Held);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(HELD).is_some());
    drop(h);
    remove_fixture(&dir);
}

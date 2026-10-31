//! The issue panel: a loaded GitHub issue with a linked branch, a Jira issue with its status,
//! epic and rich text, an Azure Boards work item with its tags and parent, a failed load, the
//! ISSUES header's + with the open issue's row, Esc closing the panel, and the Issue tracker
//! page's choice about year-like branch names. Shares the fixture
//! repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_gitflow::pushed;
use super::snapshot_tests_select::click;
use crate::msg::{ForgeMsg, IssueMsg, Msg};
use crate::state::issue::{Details, IssueView};
use crate::state::{AppState, ForgeState, PrefPage, PrefsState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Entry, EntryKind, Issue, IssueDetails, IssueFacets, Provider, Target};
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

pub(crate) fn github() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "acme".into(), project: String::new(), repo: "demo".into() }
}

fn jira() -> Target {
    Target::jira(&Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() }, "PROJ")
}

pub(crate) fn crash() -> Issue {
    Issue {
        key: "12".into(),
        title: "Crash on start when the config folder is missing".into(),
        state: "open".into(),
        kind: "bug".into(),
        assignee: Some("ada".into()),
        url: "https://github.com/acme/demo/issues/12".into(),
        facets: IssueFacets { labels: vec!["bug".into(), "ui".into()], milestone: Some("v1.2".into()) },
    }
}

fn comment(author: &str, time: i64, body: &str) -> Entry {
    Entry { author: author.into(), time, kind: EntryKind::Comment, body: body.into(), url: String::new() }
}

/// The fixture on GitHub (acme/demo) with `issue` assigned and, `with_branch`, a branch named
/// after it; the panel open for it.
pub(crate) fn with_issue(theme: ThemeChoice, name: &str, tracker: Target, issue: Issue, branch: Option<&str>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-10T10:00:00+00:00";
    git(&dir, &["remote", "add", "origin", "https://github.com/acme/demo.git"], d);
    if let Some(b) = branch {
        git(&dir, &["branch", b, "main"], d);
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    let other = if tracker.provider.is_jira() { "PROJ-14" } else { "14" };
    let mut fs = ForgeState::new(Some(github()), "origin".into(), Some(tracker));
    fs.host_status = Status::Ok;
    fs.tracker_status = Status::Ok;
    fs.my_issues = vec![issue.clone(), Issue { key: other.into(), title: "Faster search".into(), ..crash() }];
    let view = IssueView::new(issue, &fs).unwrap();
    st.session.forge = Some(fs);
    st.session.issue.view = Some(view);
    (st, dir)
}

pub(crate) fn set_details(st: &mut AppState, d: IssueDetails) {
    let v = st.session.issue.view.as_mut().unwrap();
    v.issue = d.issue.clone();
    v.details = Details::Loaded(Box::new(d));
}

/// The GitHub issue's loaded details: opened by grace on January 24, 2026, with two comments.
pub(crate) fn crash_details() -> IssueDetails {
    IssueDetails {
        issue: crash(),
        body: "Starting the app without ~/.config/demo panics.\n\nSteps:\n1. Remove the folder\n2. Start the app".into(),
        author: "grace".into(),
        created: 1_769_256_000,
        assignees: vec!["ada".into(), "linus".into()],
        priority: None,
        epic: None,
        comments: vec![
            comment("ada", 1_769_342_400, "I can reproduce this on Windows."),
            comment("linus", 1_769_860_800, "Fixed on the 12-crash-on-start branch, needs review."),
        ],
    }
}

#[test]
fn issue_panel_github_light() {
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-gh", github(), crash(), Some("12-crash-on-start"));
    set_details(&mut st, crash_details());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("issue_panel_github_light");
    assert!(h.query_by_label("No branches for this issue yet.").is_none());
    assert!(!h.get_by_label("Open in browser").is_disabled());
    h.get_by_label("Copy #12").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t == "#12")));
    h.get_by_label("Close panel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Close))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_kind_line_date_tooltip() {
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-gh-tip", github(), crash(), Some("12-crash-on-start"));
    set_details(&mut st, crash_details());
    let mut h = harness(st, false);
    h.run();
    h.get_by_label_contains("opened by").hover();
    for _ in 0..4 {
        h.step();
    }
    assert!(h.query_by_label_contains("January 24, 2026 12:00 PM").is_some(), "the kind line's hover has the full date");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_panel_jira_dark() {
    let issue = Issue {
        key: "PROJ-7".into(),
        title: "Sign-in page rejects valid passwords".into(),
        state: "In Progress".into(),
        kind: "Story".into(),
        assignee: Some("Ada Lovelace".into()),
        url: "https://acme.atlassian.net/browse/PROJ-7".into(),
        facets: IssueFacets { labels: vec!["auth".into()], milestone: Some("2.0".into()) },
    };
    let (mut st, dir) = with_issue(ThemeChoice::Dark, "issue-jira", jira(), issue.clone(), Some("feature/PROJ-7-sign-in"));
    set_details(
        &mut st,
        IssueDetails {
            issue,
            body: "Users with a + in their password can't sign in.\n\n- Seen on Chrome and Firefox\n- Started after 1.9".into(),
            author: "Grace Hopper".into(),
            created: 1_769_256_000,
            assignees: vec!["Ada Lovelace".into()],
            priority: None,
            epic: Some("PROJ-1  Accounts and sign-in".into()),
            comments: Vec::new(),
        },
    );
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("issue_panel_jira_dark");
    assert!(h.query_by_label("No comments yet.").is_some());
    assert!(h.query_by_label("Status").is_some() && h.query_by_label("Fix version").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_panel_failed() {
    let (mut st, dir) = with_issue(ThemeChoice::Dark, "issue-failed", github(), crash(), None);
    let v = st.session.issue.view.as_mut().unwrap();
    v.issue.url = String::new();
    v.details = Details::Failed("not found: Issue #12 wasn't found".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("issue_panel_failed");
    assert!(h.get_by_label("Open in browser").is_disabled(), "no link");
    assert!(h.query_by_label("No branches for this issue yet.").is_some());
    assert!(h.query_by_label("Couldn't load this issue").is_some());
    assert!(h.query_by_label("Issue #12 wasn't found").is_some(), "without the kind prefix");
    assert!(h.query_by_label("Copy details").is_some());
    h.get_by_label("Try again").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Reload))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn sidebar_issues_with_add() {
    let (st, dir) = with_issue(ThemeChoice::Light, "issue-sidebar", github(), crash(), None);
    let mut h = harness(st, false);
    h.run_steps(4);
    h.check_snapshot("sidebar_issues_with_add");
    h.get_by_label("Create issue").click();
    h.run_steps(4);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::CreateDialog))));
    assert!(h.state().0.view.sections.issues, "the + leaves the section open");

    // A click on a row opens it; its menu offers View issue first.
    h.state_mut().1.clear();
    let row = h.get_by_label("#14  Faster search").bounding_box().expect("the row");
    let center = pos2(((row.x0 + row.x1) / 2.0) as f32, ((row.y0 + row.y1) / 2.0) as f32);
    click(&mut h, center, PointerButton::Primary);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Open(k)) if k == "14")));
    click(&mut h, center, PointerButton::Secondary);
    h.get_by_label("View issue").click();
    h.run_steps(4);
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::Issue(IssueMsg::Open(k)) if k == "14")).count(), 2);
    assert!(h.state().1.iter().all(|m| !matches!(m, Msg::OpenUrl(_))), "no browser for a click");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_is_disabled_with_the_reason() {
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-sidebar-blocked", github(), crash(), None);
    st.session.issue.view = None;
    let ghe = Target { provider: Provider::GitHubEnterprise, host: "git.acme.io".into(), ..github() };
    st.session.forge.as_mut().unwrap().tracker = Some(ghe);
    let mut h = harness(st, false);
    h.run();
    assert!(h.get_by_label("Create issue").is_disabled());
    assert!(h.query_by_label("View issue").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_the_issue_panel() {
    let (st, dir) = with_issue(ThemeChoice::Dark, "issue-esc", github(), crash(), None);
    let mut h = super::snapshot_tests_esc::app_harness(st);
    h.run_steps(4);
    let msgs = super::snapshot_tests_esc::esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Close))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_panel_azure_light() {
    let azure = Target {
        provider: Provider::AzureDevOps,
        host: "dev.azure.com".into(),
        owner: "contoso".into(),
        project: "Fabrikam".into(),
        repo: "web".into(),
    };
    let issue = Issue {
        key: "5".into(),
        title: "Sign in with a work account".into(),
        state: "Active".into(),
        kind: "User Story".into(),
        assignee: Some("Ada Lovelace".into()),
        url: "https://dev.azure.com/contoso/Fabrikam/_workitems/edit/5".into(),
        facets: IssueFacets { labels: vec!["ux".into(), "needs design".into()], milestone: None },
    };
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-azure", azure, issue.clone(), None);
    set_details(
        &mut st,
        IssueDetails {
            issue,
            body: "People sign in with their work account.\n\nAcceptance:\n- Email and password\n- Single sign-on (https://contoso.example/sso)".into(),
            author: "Grace Hopper".into(),
            created: 1_769_256_000,
            assignees: vec!["Ada Lovelace".into()],
            priority: None,
            epic: Some("1  Accounts".into()),
            comments: vec![
                comment("Ada Lovelace", 1_769_342_400, "The login page mock-up is attached to the parent."),
                comment("Grace Hopper", 1_769_860_800, "SSO goes through the tenant's own page."),
            ],
        },
    );
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("issue_panel_azure_light");
    for label in ["Tags", "Assigned to", "Parent"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("Accounts").is_some());
    assert!(h.query_all_by_label_contains("AB#5").count() >= 2, "the panel and its row");
    assert!(h.query_by_label("Copy AB#5").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn bitbucket_data_center_points_to_jira_in_preferences() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "issue-bbdc-prefs");
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    st.session.forge = Some(ForgeState::new(Some(dc), "origin".into(), None));
    st.prefs = Some(PrefsState::new(PrefPage::RepoTracker));
    let mut h = harness(st, false);
    h.run();
    let help = "Bitbucket Data Center keeps no issues of its own. Connect Jira under Integrations and choose it here.";
    assert!(h.query_by_label(help).is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_tracker_year_branches() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "issue-year-branches");
    st.session.forge = Some(ForgeState::new(Some(github()), "origin".into(), Some(github())));
    st.prefs = Some(PrefsState::new(PrefPage::RepoTracker));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("prefs_tracker_year_branches");
    h.get_by_label("Read a year at the start of a branch name as an issue number").click();
    h.run();
    assert!(pushed(&h, &|m: &Msg| matches!(m, Msg::Forge(ForgeMsg::SetYearBranches(true)))));
    drop(h);
    remove_fixture(&dir);
}

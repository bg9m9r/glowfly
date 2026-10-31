//! Pull request and issue filters: the header chips, their popover, the filtered sections and
//! the filter dialog. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::{FilterMsg, Msg};
use crate::state::forge_filter_form::FilterForm;
use crate::state::forge_filters::{IssueList, MeState, OptionsState};
use crate::state::touched::Touched;
use crate::state::{AppState, ForgeState, Modal, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::Key;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::filters::{Milestone, Named, Sprint};
use glowfly_core::forge::{
    Account, Builtin, Conditions, FilterChoice, FilterKind, FilterOptions, Issue, IssueFacets, Me, PrFacets, Provider, PullRequest, Target,
};
use glowfly_core::settings::{RepoForgeFilters, ThemeChoice};
use std::path::PathBuf;

pub(crate) fn target(provider: Provider, host: &str) -> Target {
    Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn pr(id: u64, title: &str, author: &str, awaiting: &[&str]) -> PullRequest {
    PullRequest {
        id,
        title: title.into(),
        description: String::new(),
        source: format!("topic-{id}"),
        target: "main".into(),
        author: author.into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: format!("https://github.com/o/r/pull/{id}"),
        facets: PrFacets {
            author_id: author.into(),
            awaiting_ids: awaiting.iter().map(|s| s.to_string()).collect(),
            labels: vec!["ui".into()],
            ..Default::default()
        },
    }
}

/// The fixture on `provider` with four open pull requests (one waits for Ada's review), and
/// `filters` saved for the repository.
fn with_prs(theme: ThemeChoice, name: &str, provider: Provider, filters: RepoForgeFilters) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let host = if provider == Provider::Bitbucket { "bitbucket.org" } else { "github.com" };
    let mut f = ForgeState::new(Some(target(provider, host)), "origin".into(), None);
    f.host_status = Status::Ok;
    f.prs = vec![
        pr(12, "Search results page", "ada", &[]),
        pr(11, "Fix flaky login test", "grace", &["ada"]),
        pr(10, "Dark mode", "linus", &["grace"]),
        pr(9, "Update README", "grace", &[]),
    ];
    st.session.forge = Some(f);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    st.settings.set_forge_filters(&real, filters);
    (st, dir)
}

fn showing(kind: FilterKind, b: Builtin) -> RepoForgeFilters {
    let mut f = RepoForgeFilters::default();
    f.set_choice(kind, FilterChoice::Builtin(b));
    f
}

fn ada() -> Me {
    Me { id: "ada".into(), name: "Ada".into(), ..Default::default() }
}

#[test]
fn sidebar_pr_filter_chip() {
    let (mut st, dir) = with_prs(ThemeChoice::Dark, "pr-filter-chip", Provider::GitHub, showing(FilterKind::Prs, Builtin::NeedsMyReview));
    st.session.forge.as_mut().unwrap().filters.me = MeState::Known(ada());
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Pull requests filter: Needs my review").is_some());
    assert!(h.query_by_label("Needs my review · 1 of 4").is_some());
    assert!(h.query_by_label("Dark mode").is_none(), "waits for someone else");
    h.check_snapshot("sidebar_pr_filter_chip");
    drop(h);
    remove_fixture(&dir);
}

fn two_saved() -> RepoForgeFilters {
    let mut f = RepoForgeFilters::default();
    f.add(FilterKind::Prs, "UI work", Conditions { labels: vec!["ui".into()], ..Default::default() });
    f.add(FilterKind::Prs, "Ready for release", Conditions { base: "release".into(), ..Default::default() });
    f
}

#[test]
fn sidebar_pr_filter_menu() {
    let (st, dir) = with_prs(ThemeChoice::Light, "pr-filter-menu", Provider::GitHub, two_saved());
    let mut h = harness(st, false);
    h.run();
    h.get_by_label("Pull requests filter: All").click();
    h.run();
    assert!(h.query_by_label("Assigned to me").is_some_and(|n| !n.is_disabled()));
    h.get_by_label("UI work").hover();
    for _ in 0..3 {
        h.run();
    }
    assert!(h.query_by_label("New filter…").is_some());
    h.check_snapshot("sidebar_pr_filter_menu");
    h.get_by_label("Mine").click();
    h.run();
    let want =
        |m: &Msg| matches!(m, Msg::ForgeFilter(FilterMsg::Choose { kind: FilterKind::Prs, choice: FilterChoice::Builtin(Builtin::MyPrs) }));
    assert!(h.state().1.iter().any(want));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn sidebar_pr_filter_menu_bitbucket() {
    let (st, dir) = with_prs(ThemeChoice::Dark, "pr-filter-menu-bb", Provider::Bitbucket, RepoForgeFilters::default());
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Pull requests filter: All").click();
    h.run();
    assert!(h.query_by_label("Assigned to me").is_some_and(|n| n.is_disabled()));
    assert!(h.query_by_label("Needs my review").is_some_and(|n| !n.is_disabled()));
    h.check_snapshot("sidebar_pr_filter_menu_bitbucket");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn sidebar_pr_filter_finding_me() {
    let (st, dir) = with_prs(ThemeChoice::Light, "pr-filter-finding", Provider::GitHub, showing(FilterKind::Prs, Builtin::MyPrs));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Finding your GitHub account…").is_some());
    h.check_snapshot("sidebar_pr_filter_finding_me");
    drop(h);
    remove_fixture(&dir);
}

fn jira_issue(key: &str, title: &str, labels: &[&str]) -> Issue {
    Issue {
        key: key.into(),
        title: title.into(),
        state: "In Progress".into(),
        kind: "Story".into(),
        assignee: Some("Ada".into()),
        url: format!("https://acme.atlassian.net/browse/{key}"),
        facets: IssueFacets { labels: labels.iter().map(|s| s.to_string()).collect(), milestone: Some("2.0".into()) },
    }
}

fn jira() -> Target {
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    Target::jira(&account, "PROJ")
}

#[test]
fn sidebar_issue_filter_jira() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "issue-filter-jira");
    let mut f = ForgeState::new(None, String::new(), Some(jira()));
    f.tracker_status = Status::Ok;
    let mut filters = RepoForgeFilters::default();
    let sprint = Conditions { sprint: Sprint::Open, ..Default::default() };
    let id = filters.add(FilterKind::Issues, "Current sprint", sprint.clone());
    filters.set_choice(FilterKind::Issues, FilterChoice::Saved(id));
    let items = vec![jira_issue("PROJ-12", "Crash on start", &["bug"]), jira_issue("PROJ-14", "Faster search", &[])];
    f.filters.issues = Some(IssueList { choice: FilterChoice::Saved(id), conditions: sprint, items, status: Status::Ok });
    st.session.forge = Some(f);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    st.settings.set_forge_filters(&real, filters);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Jira Cloud · Current sprint").is_some());
    assert!(h.query_by_label("Issues filter: Current sprint").is_some());
    // The status bar counts the list the sidebar shows, with no "·" before the tracker.
    assert!(h.query_by_label("· 2 in Current sprint").is_some());
    assert!(h.query_by_label_contains("0 assigned").is_none());
    let bottom = f64::from(h.ctx.screen_rect().bottom()) - 26.0;
    let jira =
        h.query_all_by_label("Jira Cloud").filter_map(|n| n.bounding_box()).find(|r| r.y0 >= bottom).expect("the status bar's tracker");
    let early_dot = h.query_all_by_label("·").filter_map(|n| n.bounding_box()).any(|r| r.y0 >= bottom && r.x1 <= jira.x0);
    assert!(!early_dot, "no separator before the tracker");
    h.check_snapshot("sidebar_issue_filter_jira");
    drop(h);
    remove_fixture(&dir);
}

fn loaded(options: FilterOptions) -> OptionsState {
    OptionsState::Loaded(options)
}

#[test]
fn forge_filter_dialog_pr() {
    let (mut st, dir) = with_prs(ThemeChoice::Light, "filter-dialog-pr", Provider::GitHub, RepoForgeFilters::default());
    let f = st.session.forge.as_mut().unwrap();
    f.filters.pr_options = loaded(FilterOptions {
        labels: vec!["bug".into(), "docs".into(), "performance".into()],
        milestones: vec![Named { key: "3".into(), title: "v1.2".into() }],
        sprints: Vec::new(),
    });
    let mut form = FilterForm::new(FilterKind::Prs, Some(Provider::GitHub), &Builtin::MyPrs.conditions());
    form.name = "My UI work".into();
    form.labels = "ui".into();
    form.c.milestone = Milestone::Named { key: "3".into(), title: "v1.2".into() };
    st.modal = Some(Modal::ForgeFilter(Box::new(form)));
    render("forge_filter_dialog_pr", st, false);
    remove_fixture(&dir);
}

#[test]
fn forge_filter_dialog_issue_jira() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "filter-dialog-jira");
    let mut f = ForgeState::new(None, String::new(), Some(jira()));
    f.tracker_status = Status::Ok;
    f.filters.issue_options = loaded(FilterOptions {
        labels: vec!["backend".into(), "ui".into()],
        milestones: vec![Named { key: "2.0".into(), title: "2.0".into() }],
        sprints: vec![Named { key: "42".into(), title: "Sprint 9".into() }],
    });
    st.session.forge = Some(f);
    let mut form = FilterForm::new(FilterKind::Issues, Some(Provider::JiraCloud), &Builtin::IssuesAssignedToMe.conditions());
    form.name = "This sprint".into();
    form.c.sprint = Sprint::Named { id: 42, name: "Sprint 9".into() };
    st.modal = Some(Modal::ForgeFilter(Box::new(form)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Mentions me").is_some_and(|n| n.is_disabled()));
    h.check_snapshot("forge_filter_dialog_issue_jira");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn forge_filter_dialog_problems() {
    let (mut st, dir) = with_prs(ThemeChoice::Light, "filter-dialog-problems", Provider::GitHub, two_saved());
    st.session.forge.as_mut().unwrap().filters.pr_options = OptionsState::Loading;
    let mut form = FilterForm::new(FilterKind::Prs, Some(Provider::GitHub), &Conditions::default());
    form.name = "ui WORK".into();
    form.touched = Touched::all();
    st.modal = Some(Modal::ForgeFilter(Box::new(form)));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Another pull request filter is already called \u{201c}ui WORK\u{201d}.").is_some());
    assert!(h.query_by_label("Create filter").is_some_and(|n| n.is_disabled()));
    h.check_snapshot("forge_filter_dialog_problems");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_filter_waits_for_a_name() {
    let (mut st, dir) = with_prs(ThemeChoice::Dark, "filter-dialog-untouched", Provider::GitHub, two_saved());
    let form = FilterForm::new(FilterKind::Prs, Some(Provider::GitHub), &Conditions::default());
    st.modal = Some(Modal::ForgeFilter(Box::new(form)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Enter a name.").is_none(), "nobody typed a name yet");
    assert!(h.query_by_label("Create filter").is_some_and(|n| n.is_disabled()));
    h.press_key(Key::Enter);
    h.run();
    assert!(h.query_by_label("Enter a name.").is_some(), "a submit shows it");
    assert!(matches!(&h.state().0.modal, Some(Modal::ForgeFilter(_))), "still open");
    drop(h);
    remove_fixture(&dir);
}

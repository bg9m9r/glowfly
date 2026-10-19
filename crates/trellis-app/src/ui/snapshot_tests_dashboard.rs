//! The cross-repository dashboard: Mine and Assigned with rows from several services, the
//! Not loaded section, the dashboard over an open repository, the reading state, and the
//! messages its controls and row menus push. Its empty states are in
//! `snapshot_tests_empty_panes`.

use super::snapshot_tests::{harness, remove_fixture, render, settings, with_repo};
use super::snapshot_tests_select::click;
use super::{UiCache, avatars::Avatars, draw, escape_key, theme::color};
use crate::msg::{DashboardMsg, Msg, PrefsMsg};
use crate::state::dashboard::{AccountRow, AccountState, Basis, DashShow, LocalRepo, Phase, TargetData};
use crate::state::{AppState, PrefPage};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Key, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, Checks, Issue, Me, PrFacets, Provider, PullRequest, Reviewer, Target};
use trellis_core::settings::ThemeChoice;
use trellis_core::spell::Speller;

/// The harness's clock (2026-02-01 12:00 UTC); the last refresh was five minutes before.
const NOW: i64 = 1_769_947_200;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn path(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-dashboard-snapshots").join(name)
}

fn account(provider: Provider, host: &str) -> Account {
    Account { provider, host: host.into(), org: String::new() }
}

fn target(provider: Provider, host: &str, owner: &str, repo: &str) -> Target {
    Target { provider, host: host.into(), owner: owner.into(), project: String::new(), repo: repo.into() }
}

fn local(name: &str, url: &str) -> LocalRepo {
    let p = path(name);
    LocalRepo { path: p.clone(), real: p, name: name.into(), remote: "origin".into(), remote_url: url.into(), is_host: !url.is_empty() }
}

fn data(t: Target, locals: Vec<LocalRepo>, prs: Vec<PullRequest>, issues: Vec<Issue>) -> TargetData {
    let (wants_prs, wants_issues) = (!t.provider.is_tracker_only(), true);
    TargetData { target: t, locals, wants_prs, wants_issues, prs: Some(Ok(prs)), issues: Some(Ok(issues)) }
}

fn pr(id: u64, title: &str, author: &str, checks: Option<Checks>) -> PullRequest {
    PullRequest {
        id,
        title: title.into(),
        description: "Makes the search page faster by caching the index.".into(),
        source: format!("topic-{id}"),
        target: "main".into(),
        author: author.into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks,
        url: format!("https://example.com/pr/{id}"),
        facets: PrFacets { author_id: author.into(), ..Default::default() },
    }
}

fn awaiting(mut p: PullRequest) -> PullRequest {
    p.facets.awaiting_ids = vec!["ada".into()];
    p
}

fn vote(mut p: PullRequest, name: &str, state: &str) -> PullRequest {
    p.reviewers.push(Reviewer { name: name.into(), state: state.into(), required: false });
    p
}

fn issue(key: &str, title: &str, state: &str) -> Issue {
    let facets = Default::default();
    let url = format!("https://example.com/issues/{key}");
    Issue { key: key.into(), title: title.into(), state: state.into(), kind: "Issue".into(), assignee: Some("ada".into()), url, facets }
}

fn ok(a: Account) -> AccountRow {
    AccountRow { me: Some(Me { id: "ada".into(), name: "Ada".into() }), ..AccountRow::new(a, AccountState::Ok) }
}

/// GitHub with two repositories (one cloned twice), GitLab with one, a GitHub Enterprise server
/// that isn't connected and a Bitbucket sign-in that failed.
fn dashboard(st: &mut AppState) {
    let d = &mut st.dashboard;
    d.open = true;
    d.basis = Some(Basis { repos: vec![path("widgets")], connections: Vec::new() });
    d.fetched_at = Some(NOW - 300);
    let gh = |repo: &str| target(Provider::GitHub, "github.com", "acme", repo);
    let draft = PullRequest { draft: true, ..pr(41, "Dark mode", "ada", Some(Checks::from_counts(3, 1, 0, true))) };
    let search =
        vote(vote(pr(42, "Speed up search", "ada", Some(Checks::from_counts(6, 0, 0, true))), "grace", "approved"), "linus", "approved");
    let widgets = vec![search, draft, awaiting(pr(40, "Fix login redirect", "grace", None))];
    let api =
        vec![pr(7, "Retry webhooks", "ada", Some(Checks::from_counts(2, 0, 3, false))), awaiting(pr(8, "Rate limits", "linus", None))];
    let gl = target(Provider::GitLab, "gitlab.com", "tools", "app");
    let mr = vote(pr(3, "Upgrade the build image", "ada", None), "sam", "changes requested");
    d.targets = vec![
        data(gh("widgets"), vec![local("widgets", "https://github.com/acme/widgets"), local("widgets-fork", "")], widgets, vec![]),
        data(gh("api"), vec![local("api", "https://github.com/acme/api")], api, vec![issue("12", "Crash on start", "open")]),
        data(gl, vec![local("app", "https://gitlab.com/tools/app")], vec![mr], vec![]),
    ];
    d.accounts = vec![
        ok(account(Provider::GitHub, "github.com")),
        ok(account(Provider::GitLab, "gitlab.com")),
        AccountRow::new(account(Provider::GitHubEnterprise, "github.acme.io"), AccountState::NotConnected),
        AccountRow::new(account(Provider::Bitbucket, "bitbucket.org"), AccountState::SignInFailed("not signed in to Bitbucket".into())),
    ];
    d.no_service = vec![path("notes"), path("dotfiles")];
    d.unreadable = vec![(path("old-site"), "missing".into())];
}

pub(super) fn state(theme: ThemeChoice) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    dashboard(&mut st);
    st
}

#[test]
fn dashboard_mine() {
    render("dashboard_mine", state(ThemeChoice::Dark), true);
}

#[test]
fn dashboard_assigned_light() {
    let mut st = state(ThemeChoice::Light);
    let jira_account = account(Provider::JiraCloud, "acme.atlassian.net");
    let trello_account = account(Provider::Trello, "trello.com");
    let jira = Target::jira(&jira_account, "PROJ");
    let trello = Target::trello(&trello_account, "b1", "Roadmap");
    let d = &mut st.dashboard;
    d.show = DashShow::Assigned;
    d.filter = "search".into();
    d.targets[1].issues = Some(Ok(vec![issue("12", "Crash on start", "open"), issue("15", "Search ignores accents", "open")]));
    let jira_issues = vec![issue("PROJ-3", "Search index rebuild", "In Progress"), issue("PROJ-5", "Billing export", "To Do")];
    let mut cards = vec![issue("c1", "Search page copy", "Doing"), issue("c2", "Team offsite", "Doing")];
    for c in &mut cards {
        c.kind = "Card".into();
    }
    d.targets.push(data(jira, vec![local("widgets", "")], vec![], jira_issues));
    d.targets.push(data(trello, vec![local("api", "")], vec![], cards));
    d.accounts.insert(2, ok(jira_account));
    d.accounts.insert(3, ok(trello_account));
    render("dashboard_assigned_light", st, false);
}

#[test]
fn dashboard_over_repo() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dashboard-over");
    dashboard(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Filter (branches, PRs, issues)").is_none(), "no sidebar");
    h.check_snapshot("dashboard_over_repo");
    remove_fixture(&dir);
}

#[test]
fn dashboard_reading() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let d = &mut st.dashboard;
    (d.open, d.phase) = (true, Phase::Fetching);
    d.basis = Some(Basis { repos: vec![path("widgets")], connections: Vec::new() });
    d.accounts = vec![AccountRow::new(account(Provider::GitHub, "github.com"), AccountState::Pending)];
    d.targets =
        vec![TargetData { prs: None, issues: None, ..data(target(Provider::GitHub, "github.com", "acme", "w"), vec![], vec![], vec![]) }];
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Asking 1 service…").is_some());
    h.check_snapshot("dashboard_reading");
}

/// Two clicks in one frame: egui reads them as a double click.
fn double_click(h: &mut H, pos: egui::Pos2) {
    let events = &mut h.input_mut().events;
    events.push(egui::Event::PointerMoved(pos));
    for pressed in [true, false, true, false] {
        events.push(egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

fn pushed(h: &mut H) -> Vec<Msg> {
    std::mem::take(&mut h.state_mut().1)
}

/// Right-clicks the row labelled `row` and clicks `item` in its menu.
fn menu_item(h: &mut H, row: &str, item: &str) -> Vec<Msg> {
    let b = h.get_by_label(row).bounding_box().expect("the row");
    click(h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Secondary);
    pushed(h);
    h.get_by_label(item).click();
    h.run();
    pushed(h)
}

#[test]
fn dashboard_controls_and_menus_push_messages() {
    let mut h = harness(state(ThemeChoice::Dark), true);
    h.run();
    h.get_by_label("Review requested (2)").click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::Show(DashShow::Review)))));
    h.get_by_label("Refresh").click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::Refresh))));
    let m = menu_item(&mut h, "Speed up search", "Open in browser");
    assert!(m.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u == "https://example.com/pr/42")));
    let m = menu_item(&mut h, "Speed up search", "Copy link");
    assert!(m.iter().any(|m| matches!(m, Msg::Copy(u) if u == "https://example.com/pr/42")));
    let m = menu_item(&mut h, "Speed up search", "Show pull request");
    assert!(m.iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::OpenPr { id: 42, path: p, .. }) if *p == path("widgets"))));
    h.get_all_by_label("Connect…").next().expect("a Connect button").click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)))));
    h.get_by_label("Sign in").click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::SignIn(a)) if a.provider == Provider::Bitbucket)));

    let mut st = state(ThemeChoice::Dark);
    st.dashboard.show = DashShow::Assigned;
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("#12  Crash on start").bounding_box().expect("the issue row");
    double_click(&mut h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32));
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::OpenUrl(u) if u == "https://example.com/issues/12")));
}

#[test]
fn esc_closes_the_dashboard_and_the_repository_menu_opens_it() {
    let mut cache: Option<UiCache> = None;
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            escape_key(ctx, st, msgs);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
            });
            draw(ctx, st, cache, msgs);
        },
        (state(ThemeChoice::Dark), Vec::new()),
    );
    h.run();
    pushed(&mut h);
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::Close))));

    // With no repository open, the repository menu still offers the dashboard.
    let mut h = harness(AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now()), true);
    h.run();
    h.get_by_label("Open…").click();
    h.run();
    h.get_by_label("Dashboard").click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Dashboard(DashboardMsg::Open))));
}

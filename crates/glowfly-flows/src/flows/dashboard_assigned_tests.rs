//! Headless tests for the dashboard's Assigned: pull requests assigned to the signed-in user,
//! beside the assigned issues.

use crate::effect::{AccountReply, ForgeError, TargetReply};
use crate::flows::dashboard_tests::{deliver, fetch_of, gh, issue, path, pr, reply, run, scanned, token};
use crate::flows::state;
use crate::msg::DashboardMsg;
use crate::state::AppState;
use crate::state::dashboard::DashShow;
use crate::state::dashboard_rows::{Row, counts, sections};
use glowfly_core::forge::{Account, Issue, Me, Provider, PullRequest, Target};
use glowfly_core::settings::TrackerChoice;

fn assigned(mut p: PullRequest, ids: &[&str]) -> PullRequest {
    p.facets.assignee_ids = ids.iter().map(|s| s.to_string()).collect();
    p
}

fn ada() -> Option<Result<Me, ForgeError>> {
    Some(Ok(Me { id: "ada".into(), name: "Ada".into(), ..Default::default() }))
}

fn failed(message: &str) -> ForgeError {
    ForgeError { message: message.into(), auth: false }
}

fn answer(target: Target, prs: Result<Vec<PullRequest>, ForgeError>, issues: Result<Vec<Issue>, ForgeError>) -> TargetReply {
    TargetReply { target, prs: Some(prs), issues: Some(issues) }
}

/// Signed in to `account`, the user looked up as `me`, with these targets answered.
fn signed_in(account: Account, me: Option<Result<Me, ForgeError>>, targets: Vec<TargetReply>) -> AccountReply {
    AccountReply { auth: Ok(token(account.provider)), account, me, targets }
}

/// The rows as "pr <id>" and "issue <key>".
fn ids(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| match r {
            Row::Pr(p) => format!("pr {}", p.id),
            Row::Issue(i) => format!("issue {}", i.key),
        })
        .collect()
}

/// The first scanned target of `provider`.
fn target_of(st: &AppState, provider: Provider) -> Target {
    st.dashboard.targets.iter().find(|t| t.target.provider == provider).expect("scanned").target.clone()
}

/// The rows and the error `repo` shows under Assigned with no filter, or None when not listed.
fn assigned_repo(st: &AppState, repo: &str) -> Option<(Vec<String>, Option<String>)> {
    let s = sections(&st.dashboard, DashShow::Assigned, "");
    let r = s.iter().flat_map(|s| &s.repos).find(|r| r.data.target.repo == repo)?;
    Some((ids(&r.rows), r.error.map(str::to_owned)))
}

/// The widgets repository on GitHub, answered with these lists for the user `me`.
fn widgets_with(
    prs: Result<Vec<PullRequest>, ForgeError>,
    issues: Result<Vec<Issue>, ForgeError>,
    me: Option<Result<Me, ForgeError>>,
) -> AppState {
    let mut st = state();
    st.dashboard.generation = 1;
    deliver(&mut st, vec![scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[])]);
    let w = answer(gh("acme", "widgets"), prs, issues);
    run(&mut st, reply(1, false, vec![signed_in(gh("acme", "widgets").account(), me, vec![w])]));
    st
}

#[test]
fn assigned_lists_assigned_prs_beside_issues_and_counts_them() {
    let prs = vec![
        assigned(pr(1, "Speed up search", "ADA", &[]), &["ada"]),
        assigned(pr(2, "Fix login", "grace", &["ada"]), &["Ada"]),
        pr(3, "Docs", "linus", &[]),
    ];
    let st = widgets_with(Ok(prs), Ok(vec![issue("7", "Crash on start")]), ada());
    let d = &st.dashboard;
    assert_eq!(counts(d, ""), [1, 1, 3], "#1 is mine and assigned; #2 awaits review and is assigned");
    let rows = vec!["pr 1".to_owned(), "pr 2".into(), "issue 7".into()];
    assert_eq!(assigned_repo(&st, "widgets"), Some((rows, None)), "pull requests first, each in the service's order");
    assert_eq!(counts(d, "login")[2], 1);
    assert_eq!(counts(d, "crash")[2], 1);
    assert_eq!(counts(d, "topic-1")[2], 1, "the source branch matches");
}

#[test]
fn assigned_prs_show_for_a_repository_whose_issues_live_elsewhere() {
    let mut st = state();
    st.dashboard.generation = 1;
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    st.settings.connections.push(glowfly_core::forge::Connection {
        account: jira.clone(),
        user: "ada".into(),
        method: "access token".into(),
    });
    st.settings.set_tracker(&path("jira"), TrackerChoice::Jira { account: jira.clone(), project: "PROJ".into() });
    st.settings.set_tracker(&path("none"), TrackerChoice::None);
    st.settings.set_tracker(&path("bb"), TrackerChoice::None);
    let repos = vec![
        scanned("jira", &[("origin", "https://github.com/acme/jira-tracked")], &[]),
        scanned("none", &[("origin", "https://github.com/acme/untracked")], &[]),
        scanned("bb", &[("origin", "https://bitbucket.org/acme/site")], &[]),
    ];
    deliver(&mut st, repos);
    let (bb, proj) = (target_of(&st, Provider::Bitbucket), target_of(&st, Provider::JiraCloud));
    let to_ada = |id, title| assigned(pr(id, title, "grace", &[]), &["ada"]);
    let github = signed_in(
        gh("acme", "x").account(),
        ada(),
        vec![
            answer(gh("acme", "jira-tracked"), Ok(vec![to_ada(1, "Speed up search")]), Ok(Vec::new())),
            answer(gh("acme", "untracked"), Ok(vec![to_ada(2, "Fix login")]), Ok(Vec::new())),
        ],
    );
    let bitbucket = signed_in(bb.account(), ada(), vec![answer(bb.clone(), Ok(vec![to_ada(3, "Tidy")]), Ok(Vec::new()))]);
    let tracker = signed_in(jira, ada(), vec![answer(proj.clone(), Ok(Vec::new()), Ok(vec![issue("PROJ-4", "Crash")]))]);
    run(&mut st, reply(1, false, vec![github, bitbucket, tracker]));

    assert_eq!(assigned_repo(&st, "jira-tracked"), Some((vec!["pr 1".into()], None)));
    assert_eq!(assigned_repo(&st, "untracked"), Some((vec!["pr 2".into()], None)));
    assert_eq!(assigned_repo(&st, &proj.repo), Some((vec!["issue PROJ-4".into()], None)), "the Jira project keeps its issues");
    assert_eq!(assigned_repo(&st, &bb.repo), None, "Bitbucket pull requests have no assignee");
    assert_eq!(counts(&st.dashboard, "")[2], 3);
}

#[test]
fn assigned_shows_the_pr_error_when_issues_loaded_and_no_prs_without_the_user() {
    let one = || Ok(vec![issue("7", "Crash on start")]);
    let st = widgets_with(Err(failed("HTTP 422: validation failed")), one(), ada());
    let shown = assigned_repo(&st, "widgets");
    assert_eq!(shown, Some((vec!["issue 7".into()], Some("HTTP 422: validation failed".into()))), "the issues still list");

    let st = widgets_with(Err(failed("HTTP 422: validation failed")), Err(failed("HTTP 409")), ada());
    assert_eq!(assigned_repo(&st, "widgets"), Some((Vec::new(), Some("HTTP 409".into()))), "the issues' error first");

    // Azure DevOps pull requests have no assignee, so their list failing doesn't matter here.
    let mut st = state();
    st.dashboard.generation = 1;
    deliver(&mut st, vec![scanned("site", &[("origin", "https://dev.azure.com/acme/web/_git/site")], &[])]);
    let az = target_of(&st, Provider::AzureDevOps);
    let a = answer(az.clone(), Err(failed("HTTP 422: validation failed")), one());
    run(&mut st, reply(1, false, vec![signed_in(az.account(), ada(), vec![a])]));
    assert_eq!(assigned_repo(&st, &az.repo), Some((vec!["issue 7".into()], None)));

    // The user couldn't be looked up: the issues list, no pull request does.
    let prs = vec![assigned(pr(1, "Speed up search", "grace", &[]), &["ada"])];
    let st = widgets_with(Ok(prs), one(), Some(Err(failed("HTTP 403: forbidden"))));
    assert_eq!(assigned_repo(&st, "widgets"), Some((vec!["issue 7".into()], None)));
}

#[test]
fn showing_assigned_still_asks_for_pull_requests() {
    let mut st = state();
    st.recent = vec![path("widgets")];
    st.dashboard.show = DashShow::Assigned;
    run(&mut st, DashboardMsg::Open);
    let scan = vec![scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[])];
    let job = fetch_of(deliver(&mut st, scan)).expect("a fetch");
    let a = &job.accounts[0];
    let t = &a.targets[0];
    assert_eq!((t.target.clone(), t.prs, t.issues), (gh("acme", "widgets"), true, true), "Assigned reads the pull request list");
    assert!(a.me.is_none(), "so the runner looks the user up");
}

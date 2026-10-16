//! Headless tests for the dashboard: which repositories a refresh reads, how their scans plan
//! the service calls, replies filling the rows, sign-in, staleness and connection changes.

use super::super::tests::state;
use super::super::update;
use crate::effect::{AccountReply, DashboardEffect, Effect, FetchJob, FetchReply, ForgeError, TargetReply};
use crate::msg::{DashboardMsg, ForgeMsg, Msg};
use crate::state::AppState;
use crate::state::dashboard::{AccountRow, AccountState, DashShow, Phase};
use crate::state::dashboard_rows::{counts, sections};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::dashboard::{ScannedRepo, code_host, recognized};
use trellis_core::forge::{Account, Auth, Connection, Issue, Me, PrFacets, Provider, PullRequest, Target};
use trellis_core::settings::TrackerChoice;

/// A path that is absolute on every OS; never touched.
pub(super) fn path(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-dashboard-tests").join(name)
}

fn run(st: &mut AppState, m: DashboardMsg) -> Vec<Effect> {
    update(st, Msg::Dashboard(m), Instant::now())
}

fn scan_of(fx: &[Effect]) -> Option<(u64, Vec<PathBuf>)> {
    fx.iter().find_map(|e| match e {
        Effect::Dashboard(DashboardEffect::Scan { generation, repos, .. }) => Some((*generation, repos.clone())),
        _ => None,
    })
}

fn fetch_of(fx: Vec<Effect>) -> Option<Box<FetchJob>> {
    fx.into_iter().find_map(|e| match e {
        Effect::Dashboard(DashboardEffect::Fetch(j)) => Some(j),
        _ => None,
    })
}

pub(super) fn gh(owner: &str, repo: &str) -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: owner.into(), project: String::new(), repo: repo.into() }
}

/// What reading the folder `name` with these remotes would find.
fn scanned(name: &str, remotes: &[(&str, &str)], conns: &[Connection]) -> (PathBuf, Result<ScannedRepo, String>) {
    let p = path(name);
    let urls: Vec<(String, String)> = remotes.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect();
    let remotes = recognized(&urls, conns);
    let host = code_host(&urls, conns).and_then(|(_, t)| remotes.iter().position(|r| r.target == t));
    let s = ScannedRepo { workdir: p.clone(), real_workdir: p.clone(), folder: None, name: name.into(), remotes, host };
    (p, Ok(s))
}

fn deliver(st: &mut AppState, repos: Vec<(PathBuf, Result<ScannedRepo, String>)>) -> Vec<Effect> {
    let generation = st.dashboard.generation;
    run(st, DashboardMsg::Scanned { generation, repos })
}

fn conn(account: Account) -> Connection {
    Connection { account, user: "ada".into(), method: "access token".into() }
}

#[test]
fn open_reads_tabs_favorites_then_recent_each_once() {
    let mut st = state();
    let (a, b, c, gone) = (path("a"), path("b"), path("c"), path("gone"));
    st.settings.favorite_repos = vec![trellis_core::paths::encode(&c)];
    let c_again = PathBuf::from(format!("{}{}", c.display(), std::path::MAIN_SEPARATOR));
    st.recent = vec![a.clone(), b.clone(), c_again, gone.clone()];
    st.missing_repos = vec![gone.clone()];
    let fx = run(&mut st, DashboardMsg::Open);
    assert_eq!(scan_of(&fx), Some((1, vec![c, a, b])));
    assert_eq!(fx.len(), 1);
    assert!(st.dashboard.open);
    assert_eq!(st.dashboard.phase, Phase::Scanning);
    assert_eq!(st.dashboard.unreadable, [(gone, "missing".to_owned())]);
}

#[test]
fn open_with_no_repositories_sends_nothing() {
    let mut st = state();
    assert!(run(&mut st, DashboardMsg::Open).is_empty());
    assert_eq!(st.dashboard.phase, Phase::Idle);
    assert!(st.dashboard.open && st.dashboard.basis.is_none());
}

#[test]
fn fresh_results_are_kept_until_stale_and_refresh_always_asks() {
    let mut st = state();
    st.recent = vec![path("a")];
    let now = Instant::now();
    let open = |st: &mut AppState, at: Instant| scan_of(&update(st, Msg::Dashboard(DashboardMsg::Open), at)).is_some();
    let fresh = |st: &mut AppState| {
        st.dashboard.phase = Phase::Idle;
        st.dashboard.fetched = Some(now);
    };
    assert!(open(&mut st, now));
    fresh(&mut st);
    run(&mut st, DashboardMsg::Close);
    assert!(!open(&mut st, now), "fresh");
    assert!(open(&mut st, now + crate::state::FORGE_REFRESH_INTERVAL), "too old");
    // Another connection, or another repository, makes it stale too.
    fresh(&mut st);
    st.settings.connections.push(conn(gh("o", "r").account()));
    assert!(open(&mut st, now));
    fresh(&mut st);
    assert!(!open(&mut st, now));
    st.recent.push(path("b"));
    assert!(open(&mut st, now));
    let g = st.dashboard.generation;
    assert_eq!(scan_of(&run(&mut st, DashboardMsg::Refresh)).map(|(g, _)| g), Some(g + 1));
}

#[test]
fn clones_of_one_repository_share_a_target_and_guessed_hosts_are_not_asked() {
    let mut st = state();
    st.dashboard.generation = 1;
    let repos = vec![
        scanned("w1", &[("origin", "https://github.com/acme/widgets.git")], &[]),
        scanned("w2", &[("origin", "git@github.com:acme/widgets.git")], &[]),
        scanned("ent", &[("origin", "https://github.acme.io/team/api.git")], &[]),
        scanned("local", &[("origin", "../elsewhere.git")], &[]),
    ];
    let job = fetch_of(deliver(&mut st, repos.clone())).expect("a fetch");
    assert_eq!((job.cwd.clone(), job.interactive, job.partial), (AppState::outside_dir(), false, false));
    assert_eq!(job.accounts.len(), 1, "only github.com");
    let t = &job.accounts[0].targets;
    assert_eq!((t.len(), t[0].target.clone(), t[0].prs, t[0].issues), (1, gh("acme", "widgets"), true, true));
    assert!(!job.accounts[0].connected);
    let d = &st.dashboard;
    assert_eq!(d.targets[0].locals.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["w1", "w2"]);
    assert!(d.targets[0].locals.iter().all(|l| l.is_host));
    let ent = d.accounts.iter().find(|a| a.account.host == "github.acme.io").unwrap();
    assert_eq!(ent.state, AccountState::NotConnected);
    assert_eq!(d.no_service, [path("local")]);
    assert_eq!(d.phase, Phase::Fetching);

    // Connected, the server is asked too.
    let ent_account = Account { provider: Provider::GitHubEnterprise, host: "github.acme.io".into(), org: String::new() };
    st.settings.connections.push(conn(ent_account.clone()));
    let job = fetch_of(deliver(&mut st, repos)).unwrap();
    let ent = job.accounts.iter().find(|a| a.account == ent_account).expect("planned");
    assert!(ent.connected);
}

#[test]
fn a_jira_tracker_takes_the_issues_and_is_shared_by_its_repositories() {
    let mut st = state();
    st.dashboard.generation = 1;
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    st.settings.connections.push(conn(jira.clone()));
    for name in ["j1", "j2"] {
        st.settings.set_tracker(&path(name), TrackerChoice::Jira { account: jira.clone(), project: "PROJ".into() });
    }
    let repos = vec![
        scanned("j1", &[("origin", "https://github.com/acme/one")], &[]),
        scanned("j2", &[("origin", "https://github.com/acme/two")], &[]),
    ];
    let job = fetch_of(deliver(&mut st, repos)).unwrap();
    let gh_job = job.accounts.iter().find(|a| a.account.provider == Provider::GitHub).unwrap();
    assert!(gh_job.targets.iter().all(|t| t.prs && !t.issues), "{:?}", gh_job.targets);
    let jira_job = job.accounts.iter().find(|a| a.account == jira).unwrap();
    assert_eq!(jira_job.targets.len(), 1, "one Jira project for both");
    let j = &jira_job.targets[0];
    assert_eq!((j.target.clone(), j.prs, j.issues), (Target::jira(&jira, "PROJ"), false, true));
    let jira_data = st.dashboard.targets.iter().find(|t| t.target.provider == Provider::JiraCloud).unwrap();
    assert_eq!(jira_data.locals.len(), 2);
    // Nothing here is trusted, yet every repository is planned: only remote URLs were read.
    assert!(!st.trusts(&path("j1")));
}

#[test]
fn targets_past_the_limit_are_counted_not_asked() {
    let mut st = state();
    st.dashboard.generation = 1;
    let urls: Vec<String> = (0..60).map(|i| format!("https://github.com/acme/r{i}")).collect();
    let repos = (0..60).map(|i| scanned(&format!("r{i}"), &[("origin", &urls[i])], &[])).collect();
    let job = fetch_of(deliver(&mut st, repos)).unwrap();
    assert_eq!(job.accounts[0].targets.len(), 50);
    assert_eq!(job.accounts[0].targets[49].target.repo, "r49");
    assert_eq!(st.dashboard.over_limit, 10);
}

pub(super) fn pr(id: u64, title: &str, author: &str, awaiting: &[&str]) -> PullRequest {
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
        merge_status: None,
        checks: None,
        url: format!("https://github.com/acme/widgets/pull/{id}"),
        facets: PrFacets { author_id: author.into(), awaiting_ids: awaiting.iter().map(|s| s.to_string()).collect(), ..Default::default() },
    }
}

fn issue(key: &str, title: &str) -> Issue {
    let facets = Default::default();
    Issue { key: key.into(), title: title.into(), state: "open".into(), kind: "Issue".into(), assignee: None, url: String::new(), facets }
}

fn token(p: Provider) -> Auth {
    Auth::from_token(p, None, "t", "test")
}

fn reply(generation: u64, partial: bool, accounts: Vec<AccountReply>) -> DashboardMsg {
    DashboardMsg::Fetched(Box::new(FetchReply { generation, partial, fetched_at: 1_000, accounts }))
}

fn gitlab() -> Account {
    Account { provider: Provider::GitLab, host: "gitlab.com".into(), org: String::new() }
}

/// GitHub (widgets with three pull requests and an issue, api whose list failed, docs with no
/// pull requests) and a GitLab account whose sign-in failed.
fn filled(st: &mut AppState) {
    st.dashboard.generation = 1;
    let repos = vec![
        scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[]),
        scanned("api", &[("origin", "https://github.com/acme/api")], &[]),
        scanned("docs", &[("origin", "https://github.com/acme/docs")], &[]),
        scanned("app", &[("origin", "https://gitlab.com/g/app")], &[]),
    ];
    deliver(st, repos);
    st.dashboard.auth.insert(gitlab().key(), token(Provider::GitLab));
    let t = |target: Target, prs, issues| TargetReply { target, prs: Some(prs), issues: Some(issues) };
    let widgets = vec![pr(1, "Speed up search", "ADA", &[]), pr(2, "Fix login", "grace", &["ada"]), pr(3, "Docs", "linus", &["ada"])];
    let github = AccountReply {
        account: gh("acme", "widgets").account(),
        auth: Ok(token(Provider::GitHub)),
        me: Some(Ok(Me { id: "ada".into(), name: "Ada".into() })),
        targets: vec![
            t(gh("acme", "widgets"), Ok(widgets), Ok(vec![issue("7", "Crash on start")])),
            t(gh("acme", "api"), Err(ForgeError { message: "HTTP 502: bad gateway".into(), auth: false }), Ok(vec![issue("9", "Slow")])),
            t(gh("acme", "docs"), Ok(Vec::new()), Ok(Vec::new())),
        ],
    };
    let gl = AccountReply { account: gitlab(), auth: Err("not signed in to GitLab".into()), me: None, targets: Vec::new() };
    run(st, reply(1, false, vec![github, gl]));
}

#[test]
fn replies_fill_the_rows_and_failures_are_kept_apart() {
    let mut st = state();
    filled(&mut st);
    let d = &st.dashboard;
    assert_eq!(counts(d, ""), [1, 2, 2]);
    assert_eq!(counts(d, "fix"), [0, 1, 0]);
    let gl = d.account(&gitlab()).unwrap();
    assert_eq!(gl.state, AccountState::SignInFailed("not signed in to GitLab".into()));
    assert!(!d.auth.contains_key(&gitlab().key()), "the failed credential is dropped");
    assert!(d.auth.contains_key(&gh("o", "r").account().key()));
    let mine = sections(d, DashShow::Mine, "");
    let api = mine[0].repos.iter().find(|r| r.data.target.repo == "api").unwrap();
    assert_eq!(api.error, Some("HTTP 502: bad gateway"));
    let assigned = sections(d, DashShow::Assigned, "");
    assert!(assigned[0].repos.iter().any(|r| r.data.target.repo == "api" && r.rows.len() == 1), "its issues still list");
    assert_eq!((d.fetched_at, d.fetched.is_some(), d.phase), (Some(1_000), true, Phase::Idle));

    // Results for an older refresh change nothing.
    run(&mut st, reply(0, false, Vec::new()));
    run(&mut st, DashboardMsg::Scanned { generation: 0, repos: Vec::new() });
    assert_eq!(counts(&st.dashboard, ""), [1, 2, 2]);
    assert_eq!(st.dashboard.targets.len(), 4);
}

#[test]
fn a_filter_keeps_matching_rows_and_hides_empty_repositories() {
    let mut st = state();
    filled(&mut st);
    let d = &st.dashboard;
    let repos = |f: &str| -> Vec<String> {
        sections(d, DashShow::Review, f).iter().flat_map(|s| &s.repos).map(|r| r.data.target.repo.clone()).collect()
    };
    assert_eq!(repos(""), ["widgets", "api", "docs"]);
    assert_eq!(repos("fix"), ["widgets", "api"], "api keeps its error; docs has nothing to show");
    assert_eq!(sections(d, DashShow::Review, "fix")[0].repos[0].rows.len(), 1);
    assert_eq!(sections(d, DashShow::Review, "widgets")[0].repos[0].rows.len(), 2, "repository names match too");
}

#[test]
fn sign_in_asks_one_account_and_replaces_only_its_targets() {
    let mut st = state();
    filled(&mut st);
    let job = fetch_of(run(&mut st, DashboardMsg::SignIn(gitlab()))).expect("a fetch");
    assert!(job.interactive && job.partial);
    assert_eq!((job.accounts.len(), job.accounts[0].targets[0].target.host.as_str()), (1, "gitlab.com"));
    assert_eq!(st.dashboard.account(&gitlab()).unwrap().state, AccountState::Pending);
    let app = job.accounts[0].targets[0].target.clone();
    let me = Some(Ok(Me { id: "ada".into(), name: "Ada".into() }));
    let targets = vec![TargetReply { target: app, prs: Some(Ok(vec![pr(5, "Add cache", "ada", &[])])), issues: Some(Ok(Vec::new())) }];
    let gl = AccountReply { account: gitlab(), auth: Ok(token(Provider::GitLab)), me, targets };
    run(&mut st, DashboardMsg::Fetched(Box::new(FetchReply { generation: 1, partial: true, fetched_at: 5_000, accounts: vec![gl] })));
    let d = &st.dashboard;
    assert_eq!(d.account(&gitlab()).unwrap().state, AccountState::Ok);
    assert_eq!(counts(d, "")[0], 2, "GitHub's row is still there, plus GitLab's");
    assert_eq!(d.fetched_at, Some(1_000), "a partial reply keeps the update time");
    assert_eq!(d.phase, Phase::Idle);
}

#[test]
fn connecting_or_disconnecting_drops_the_credential_and_refreshes_while_open() {
    let mut st = state();
    st.recent = vec![path("a")];
    let account = gh("o", "r").account();
    let connected = |account: &Account| {
        Msg::Forge(ForgeMsg::Connected { account: account.clone(), method: "access token".into(), token: None, result: Ok("ada".into()) })
    };
    let cached = |st: &mut AppState| {
        st.dashboard.auth.insert(account.key(), token(Provider::GitHub));
        st.dashboard.fetched = Some(Instant::now());
        let me = Some(Me { id: "ada".into(), name: "Ada".into() });
        st.dashboard.accounts = vec![AccountRow { me, ..AccountRow::new(account.clone(), AccountState::Ok) }];
    };
    // The credential, the signed-in user (the new connection may be someone else) and the
    // results' freshness all go.
    let dropped = |st: &AppState| {
        let d = &st.dashboard;
        !d.auth.contains_key(&account.key()) && d.fetched.is_none() && d.account(&account).is_some_and(|a| a.me.is_none())
    };
    for open in [false, true] {
        st.dashboard.open = open;
        cached(&mut st);
        let fx = update(&mut st, connected(&account), Instant::now());
        assert!(dropped(&st));
        assert_eq!(scan_of(&fx).is_some(), open, "open: {open}");
        cached(&mut st);
        let fx = update(&mut st, Msg::Forge(ForgeMsg::Disconnect(account.clone())), Instant::now());
        assert!(dropped(&st));
        assert_eq!(scan_of(&fx).is_some(), open, "open: {open}");
    }
    // The next refresh looks the user up again.
    st.dashboard.open = true;
    cached(&mut st);
    update(&mut st, connected(&account), Instant::now());
    let job = fetch_of(deliver(&mut st, vec![scanned("a", &[("origin", "https://github.com/o/r")], &[])])).unwrap();
    assert!(job.accounts[0].me.is_none() && job.accounts[0].auth.is_none());
}

#[test]
fn a_scan_with_nothing_to_ask_counts_as_fresh() {
    let mut st = state();
    st.recent = vec![path("local")];
    let now = Instant::now();
    let fx = update(&mut st, Msg::Dashboard(DashboardMsg::Open), now);
    assert!(scan_of(&fx).is_some());
    let fx = deliver(&mut st, vec![scanned("local", &[("origin", "../elsewhere.git")], &[])]);
    assert!(fetch_of(fx).is_none());
    assert_eq!(st.dashboard.phase, Phase::Idle);
    assert!(st.dashboard.fetched.is_some() && st.dashboard.fetched_at.is_some());
    run(&mut st, DashboardMsg::Close);
    assert!(update(&mut st, Msg::Dashboard(DashboardMsg::Open), now).is_empty(), "reopening sends nothing");
}

#[test]
fn no_dashboard_message_is_kept_for_an_inactive_tab() {
    let msgs = [
        DashboardMsg::Open,
        DashboardMsg::Close,
        DashboardMsg::Refresh,
        DashboardMsg::Show(DashShow::Review),
        DashboardMsg::SignIn(gitlab()),
        DashboardMsg::OpenRepo(path("a")),
        DashboardMsg::OpenPr { path: path("a"), target: gh("o", "r"), id: 1 },
        DashboardMsg::Scanned { generation: 1, repos: Vec::new() },
        reply(1, false, Vec::new()),
    ];
    for m in msgs {
        assert!(m.on_inactive().is_none());
    }
}

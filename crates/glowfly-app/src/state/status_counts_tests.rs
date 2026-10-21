use super::*;
use glowfly_core::forge::filters::Sprint;
use glowfly_core::forge::{Account, Conditions, Issue, IssueFacets, PrFacets, Provider, PullRequest, Target};

#[test]
fn open_requests_text_counts_in_the_language() {
    assert_eq!(open_requests_text(Lang::En, "pull request", 1), "1 open pull request");
    assert_eq!(open_requests_text(Lang::En, "merge request", 2), "2 open merge requests");
    assert_eq!(open_requests_text(Lang::En, "pull request", 0), "0 open pull requests");
    let de = open_requests_text(Lang::De, "pull request", 2);
    assert_eq!(de, Lang::De.trn("status.open_pull_requests", 2, &[]));
    assert!(de.starts_with("2 ") && de != "2 open pull requests", "{de}");
    assert_eq!(open_requests_text(Lang::De, "merge request", 1), Lang::De.trn("status.open_merge_requests", 1, &[]));
}

fn host(provider: Provider, host: &str) -> Target {
    Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn pr(id: u64, labels: &[&str]) -> PullRequest {
    PullRequest {
        id,
        title: format!("P{id}"),
        description: String::new(),
        source: format!("b{id}"),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: PrFacets { author_id: "ada".into(), labels: labels.iter().map(|s| s.to_string()).collect(), ..Default::default() },
    }
}

fn issue(key: &str) -> Issue {
    Issue {
        key: key.into(),
        title: format!("I{key}"),
        state: "open".into(),
        kind: "Story".into(),
        assignee: None,
        url: String::new(),
        facets: IssueFacets::default(),
    }
}

fn with_prs(provider: Provider, name: &str) -> ForgeState {
    let mut fs = ForgeState::new(Some(host(provider, name)), "origin".into(), None);
    fs.host_status = Status::Ok;
    fs.prs = vec![pr(3, &["ui"]), pr(2, &[]), pr(1, &["docs"])];
    fs
}

#[test]
fn pull_requests_follow_the_sidebar_filter() {
    let none = RepoForgeFilters::default();
    let gh = with_prs(Provider::GitHub, "github.com");
    assert_eq!(prs_text(Lang::En, &gh, &none).as_deref(), Some("3 open pull requests"));
    let gl = with_prs(Provider::GitLab, "gitlab.com");
    assert_eq!(prs_text(Lang::En, &gl, &none).as_deref(), Some("3 open merge requests"));

    let mut ui = RepoForgeFilters::default();
    let id = ui.add(FilterKind::Prs, "UI work", Conditions { labels: vec!["ui".into()], ..Default::default() });
    ui.set_choice(FilterKind::Prs, FilterChoice::Saved(id));
    assert_eq!(prs_text(Lang::En, &gh, &ui).as_deref(), Some("1 in UI work"));
}

fn jira() -> ForgeState {
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let mut fs = ForgeState::new(None, String::new(), Some(Target::jira(&account, "PROJ")));
    fs.tracker_status = Status::Ok;
    fs
}

#[test]
fn issues_follow_the_sidebar_filter() {
    let mut fs = jira();
    fs.my_issues = vec![issue("PROJ-1")];
    assert_eq!(issues_text(Lang::En, &fs, &RepoForgeFilters::default()).as_deref(), Some("1 assigned"));

    let mut fs = jira();
    let mut filters = RepoForgeFilters::default();
    let sprint = Conditions { sprint: Sprint::Open, ..Default::default() };
    let id = filters.add(FilterKind::Issues, "Current sprint", sprint.clone());
    filters.set_choice(FilterKind::Issues, FilterChoice::Saved(id));
    let items = vec![issue("PROJ-12"), issue("PROJ-14")];
    let list = |status| IssueList { choice: FilterChoice::Saved(id), conditions: sprint.clone(), items: items.clone(), status };
    fs.filters.issues = Some(list(Status::Ok));
    assert_eq!(issues_text(Lang::En, &fs, &filters).as_deref(), Some("2 in Current sprint"));
    // Still loading (a refresh keeps the last answer's items): no count yet.
    fs.filters.issues = Some(list(Status::Idle));
    assert_eq!(issues_text(Lang::En, &fs, &filters), None);
    fs.filters.issues = None;
    assert_eq!(issues_text(Lang::En, &fs, &filters), None);
}

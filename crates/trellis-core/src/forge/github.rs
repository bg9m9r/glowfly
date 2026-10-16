//! GitHub and GitHub Enterprise Server (REST API v3).

use super::github_checks;
pub(super) use super::github_issues::{ApiIssue, to_issue};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, Checks, Conditions, CreateOptions, FilterKind, FilterOptions, Issue, IssueDetails, Me, NewIssue, NewPullRequest,
    PrFacets, Provider, PullRequest, Reviewer, Target, enc, get, github_issues, page, post,
};
use crate::error::Result;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub(super) static GITHUB: ProviderInfo =
    ProviderInfo { name: "GitHub", fixed_host: Some("github.com"), host_hint: "", ..GITHUB_ENTERPRISE };

pub(super) static GITHUB_ENTERPRISE: ProviderInfo = ProviderInfo {
    name: "GitHub Enterprise Server",
    host_hint: "e.g., github.mycompany.com",
    token_help: "A personal access token with the repo scope (classic), or a fine-grained token with Pull requests and Issues read & write.",
    env_var: Some("GITHUB_TOKEN"),
    close_keyword: Some("Closes"),
    api_headers: &[("X-GitHub-Api-Version", "2022-11-28")],
    lists_forks: true,
    creates_forks: true,
    adds_ssh_keys: true,
    issues_in_app: true,
    service: &GitHub,
    ..DEFAULTS
};

/// Pull requests whose reviews and checks are asked for in one GraphQL call.
const DETAIL_BATCH: usize = 50;

pub(super) fn api(host: &str, provider: Provider) -> String {
    if provider == Provider::GitHub { "https://api.github.com".into() } else { format!("https://{host}/api/v3") }
}

pub(super) fn graphql_url(t: &Target) -> String {
    if t.provider == Provider::GitHub { "https://api.github.com/graphql".into() } else { format!("https://{}/api/graphql", t.host) }
}

pub(super) fn repo_api(t: &Target) -> String {
    format!("{}/repos/{}/{}", api(&t.host, t.provider), enc(&t.owner), enc(&t.repo))
}

#[derive(Deserialize)]
pub(super) struct User {
    pub(super) login: String,
    #[serde(default)]
    pub(super) name: Option<String>,
}

pub(super) fn me(host: &str, provider: Provider, auth: &Auth) -> Result<User> {
    get(auth, &format!("{}/user", api(host, provider)))
}

#[derive(Deserialize)]
struct Ref {
    #[serde(rename = "ref")]
    name: String,
    #[serde(default)]
    sha: String,
}

#[derive(Deserialize)]
pub(super) struct Login {
    pub(super) login: String,
}

#[derive(Deserialize)]
pub(super) struct Title {
    pub(super) title: String,
}

#[derive(Deserialize)]
struct ApiPr {
    number: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    html_url: String,
    user: Login,
    head: Ref,
    base: Ref,
    #[serde(default)]
    requested_reviewers: Vec<Login>,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    assignees: Vec<Login>,
    #[serde(default)]
    milestone: Option<Title>,
}

#[derive(Deserialize)]
struct Review {
    /// `user` in REST answers, `author` in GraphQL ones; null for deleted accounts.
    #[serde(alias = "author")]
    user: Option<Login>,
    state: String,
}

/// What the GraphQL call adds to a REST pull request.
#[derive(Default)]
struct Details {
    reviews: Vec<Review>,
    checks: Option<Checks>,
}

fn convert(p: ApiPr, d: Details) -> PullRequest {
    // Latest review per reviewer wins; comments don't change a vote.
    let mut states: Vec<(String, String)> = Vec::new();
    for r in d.reviews {
        let (Some(user), state) = (r.user, r.state) else { continue };
        let label = match state.as_str() {
            "APPROVED" => "approved",
            "CHANGES_REQUESTED" => "changes requested",
            "DISMISSED" => "no vote",
            _ => continue,
        };
        states.retain(|(u, _)| *u != user.login);
        states.push((user.login, label.into()));
    }
    let awaiting_ids: Vec<String> = p.requested_reviewers.iter().map(|r| r.login.clone()).collect();
    for r in p.requested_reviewers {
        if !states.iter().any(|(u, _)| *u == r.login) {
            states.push((r.login, "requested".into()));
        }
    }
    let facets = PrFacets {
        author_id: p.user.login.clone(),
        assignee_ids: p.assignees.into_iter().map(|a| a.login).collect(),
        reviewer_ids: states.iter().map(|(u, _)| u.clone()).collect(),
        awaiting_ids,
        labels: p.labels.into_iter().map(|l| l.name).collect(),
        milestone: p.milestone.map(|m| m.title),
    };
    PullRequest {
        id: p.number,
        title: p.title,
        description: p.body.unwrap_or_default(),
        source: p.head.name,
        target: p.base.name,
        author: p.user.login,
        draft: p.draft,
        reviewers: states.into_iter().map(|(name, state)| Reviewer { name, state, required: false }).collect(),
        head: Some(p.head.sha).filter(|s| !s.is_empty()),
        merge_status: None,
        checks: d.checks,
        url: p.html_url,
        facets,
    }
}

/// The GraphQL query for one batch of pull requests' reviews and, `with_checks`, their checks.
fn query(chunk: &[u64], with_checks: bool) -> String {
    let checks = if with_checks { github_checks::ROLLUP_FIELDS } else { "" };
    let fields: String = chunk
        .iter()
        .map(|n| format!("p{n}: pullRequest(number: {n}) {{ reviews(first: 100) {{ nodes {{ author {{ login }} state }} }} {checks} }} "))
        .collect();
    format!("query($owner: String!, $repo: String!) {{ repository(owner: $owner, name: $repo) {{ {fields}}} }}")
}

/// Each pull request's reviews and checks, asked for in a few GraphQL calls rather than REST
/// calls per pull request. Pull requests for which neither can be read are left out (and show no
/// votes or checks). A server that doesn't know the check fields is asked again without them.
fn details(t: &Target, auth: &Auth, numbers: &[u64]) -> HashMap<u64, Details> {
    let ask = |chunk: &[u64], with_checks: bool| {
        let body = json!({ "query": query(chunk, with_checks), "variables": { "owner": t.owner, "repo": t.repo } });
        post::<serde_json::Value>(auth, &graphql_url(t), body)
    };
    let mut out = HashMap::new();
    let mut with_checks = true;
    for chunk in numbers.chunks(DETAIL_BATCH) {
        let Ok(mut answer) = ask(chunk, with_checks) else { break };
        if with_checks && answer["data"]["repository"].is_null() && answer["errors"].is_array() {
            // Rejected outright (an older server without the rollup field): drop the checks.
            with_checks = false;
            let Ok(again) = ask(chunk, false) else { break };
            answer = again;
        }
        let repo = &answer["data"]["repository"];
        for n in chunk {
            let pr = &repo[format!("p{n}")];
            let reviews = serde_json::from_value::<Vec<Review>>(pr["reviews"]["nodes"].clone()).ok();
            let checks = if with_checks { github_checks::from_pr(pr) } else { None };
            if reviews.is_some() || checks.is_some() {
                out.insert(*n, Details { reviews: reviews.unwrap_or_default(), checks });
            }
        }
    }
    out
}

#[derive(Deserialize)]
pub(super) struct Label {
    pub(super) name: String,
}

/// The GitHub [`Forge`](super::Forge).
pub struct GitHub;

impl super::Forge for GitHub {
    fn issue_url(&self, t: &Target, key: &str) -> String {
        // A GitHub owner is one path segment.
        format!("https://{}/{}/{}/issues/{}", t.host, enc(&t.owner), enc(&t.repo), enc(key))
    }

    fn token_url(&self, account: &Account) -> Option<String> {
        Some(format!("https://{}/settings/tokens", account.host))
    }

    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        let u = me(&account.host, account.provider, auth)?;
        Ok(u.name.filter(|n| !n.is_empty()).unwrap_or(u.login))
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let prs: Vec<ApiPr> = page::by_link(auth, format!("{}/pulls?state=open&per_page=100", repo_api(t)))?;
        let numbers: Vec<u64> = prs.iter().map(|p| p.number).collect();
        let mut details = details(t, auth, &numbers);
        Ok(prs
            .into_iter()
            .map(|p| {
                let d = details.remove(&p.number).unwrap_or_default();
                convert(p, d)
            })
            .collect())
    }

    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Repo {
            default_branch: Option<String>,
        }
        Ok(get::<Repo>(auth, &repo_api(t))?.default_branch)
    }

    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        let body = json!({ "title": pr.title, "head": pr.source, "base": pr.target, "body": pr.description, "draft": pr.draft });
        let created: ApiPr = post(auth, &format!("{}/pulls", repo_api(t)), body)?;
        Ok(convert(created, Details::default()))
    }

    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        let login = me(&t.host, t.provider, auth)?.login;
        let url = format!("{}/issues?state=open&assignee={}&per_page=100&sort=updated", repo_api(t), enc(&login));
        let items: Vec<ApiIssue> = page::by_link(auth, url)?;
        Ok(items.into_iter().filter(|i| i.pull_request.is_none()).map(to_issue).collect())
    }

    fn forks(&self, t: &Target, auth: &Auth) -> Result<super::ForkList> {
        super::forks::github::list(t, auth)
    }

    fn me(&self, t: &Target, auth: &Auth) -> Result<Me> {
        super::filters::github::me(t, auth)
    }

    fn find_issues(&self, t: &Target, auth: &Auth, c: &Conditions, me: Option<&Me>) -> Result<Vec<Issue>> {
        super::filters::github::find_issues(t, auth, c, me)
    }

    fn filter_options(&self, t: &Target, auth: &Auth, _kind: FilterKind) -> Result<FilterOptions> {
        super::filters::github::options(t, auth)
    }

    fn my_fork(&self, t: &Target, auth: &Auth) -> Result<super::ForkLookup> {
        super::forks::github::my_fork(t, auth)
    }

    fn create_fork(&self, t: &Target, auth: &Auth, wait: &super::Wait) -> Result<super::CreatedFork> {
        super::forks::github::create(t, auth, wait)
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[super::TipQuery]) -> Result<HashMap<String, Checks>> {
        github_checks::commit_rollups(t, auth, tips)
    }

    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        let mut out = Vec::new();
        for k in keys.iter().filter(|k| k.parse::<u64>().is_ok()).take(30) {
            match get::<ApiIssue>(auth, &format!("{}/issues/{k}", repo_api(t))) {
                Ok(i) => out.push(to_issue(i)),
                Err(e) if e.is_not_found() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    fn issue_details(&self, t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
        github_issues::details(t, auth, key)
    }

    fn issue_create_options(&self, t: &Target, auth: &Auth, _project: &str) -> Result<CreateOptions> {
        github_issues::options(t, auth)
    }

    fn create_issue(&self, t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
        github_issues::create(t, auth, new)
    }
}

#[cfg(test)]
mod tests {
    use super::super::IssueFacets;
    use super::*;

    #[test]
    fn converts_pull_requests_with_reviews() {
        let p: ApiPr = serde_json::from_str(
            r#"{"number":4,"title":"Add search","body":null,"draft":false,"html_url":"https://github.com/o/r/pull/4",
            "user":{"login":"ada"},"head":{"ref":"feature/search","sha":"abcdef1234567"},"base":{"ref":"main","sha":"x"},
            "requested_reviewers":[{"login":"linus"}]}"#,
        )
        .unwrap();
        let reviews: Vec<Review> = serde_json::from_str(
            r#"[{"user":{"login":"grace"},"state":"CHANGES_REQUESTED"},{"user":{"login":"grace"},"state":"APPROVED"},{"user":{"login":"bob"},"state":"COMMENTED"}]"#,
        )
        .unwrap();
        let pr = convert(p, Details { reviews, checks: None });
        assert_eq!((pr.id, pr.source.as_str(), pr.target.as_str()), (4, "feature/search", "main"));
        assert_eq!(pr.approvals(), 1);
        assert_eq!(pr.reviewers.len(), 2);
        assert!(pr.head_matches("abcdef1234567"));
        let issues: Vec<ApiIssue> = serde_json::from_str(
            r#"[{"number":9,"title":"Crash","state":"open","html_url":"u","labels":[{"name":"bug"}],"assignee":{"login":"ada"}},
            {"number":4,"title":"PR","state":"open","html_url":"u","pull_request":{"url":"x"}}]"#,
        )
        .unwrap();
        let mine: Vec<Issue> = issues.into_iter().filter(|i| i.pull_request.is_none()).map(to_issue).collect();
        assert_eq!(mine.len(), 1);
        assert_eq!((mine[0].key.as_str(), mine[0].kind.as_str()), ("9", "bug"));
        assert_eq!(api("github.acme.io", Provider::GitHubEnterprise), "https://github.acme.io/api/v3");
    }

    #[test]
    fn owner_and_repo_are_encoded_in_api_paths() {
        // %2F and %2E%2E in the remote URL are decoded when it is parsed; they must not become
        // real path separators or parent-directory steps in API calls.
        let t = super::super::parse_remote("https://github.com/..%2F..%2Fuser/r%3Fx", &[]).unwrap();
        assert_eq!(repo_api(&t), "https://api.github.com/repos/..%2F..%2Fuser/r%3Fx");
        let t = super::super::parse_remote("https://github.com/%2E%2E/r", &[]).unwrap();
        assert_eq!(repo_api(&t), "https://api.github.com/repos/%2E%2E/r");
        assert_eq!(t.issue_url("1/../x"), "https://github.com/%2E%2E/r/issues/1%2F..%2Fx");
    }

    #[test]
    fn fills_filter_facets() {
        let p: ApiPr = serde_json::from_str(
            r#"{"number":4,"title":"T","html_url":"u","user":{"login":"ada"},"head":{"ref":"x","sha":""},"base":{"ref":"main","sha":""},
            "requested_reviewers":[{"login":"linus"}],"labels":[{"name":"bug"}],"assignees":[{"login":"grace"}],"milestone":{"title":"v1"}}"#,
        )
        .unwrap();
        let reviews: Vec<Review> = serde_json::from_str(r#"[{"user":{"login":"bob"},"state":"APPROVED"}]"#).unwrap();
        let f = convert(p, Details { reviews, checks: None }).facets;
        assert_eq!(
            (f.author_id.as_str(), f.assignee_ids.as_slice(), f.milestone.as_deref()),
            ("ada", &["grace".to_owned()][..], Some("v1"))
        );
        assert_eq!(
            (f.reviewer_ids, f.awaiting_ids, f.labels),
            (vec!["bob".into(), "linus".into()], vec!["linus".into()], vec!["bug".into()])
        );
        let i: ApiIssue = serde_json::from_str(
            r#"{"number":9,"title":"C","state":"open","html_url":"u","labels":[{"name":"bug"},{"name":"ui"}],"milestone":{"title":"v2"}}"#,
        )
        .unwrap();
        assert_eq!(to_issue(i).facets, IssueFacets { labels: vec!["bug".into(), "ui".into()], milestone: Some("v2".into()) });
    }
}

//! GitHub and GitHub Enterprise Server (REST API v3).

use super::provider::{DEFAULTS, ProviderInfo};
use super::{Account, Auth, Issue, NewPullRequest, Provider, PullRequest, Reviewer, Target, enc, get, page, post};
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
    service: &GitHub,
    ..DEFAULTS
};

/// Pull requests whose reviews are asked for in one GraphQL call.
const REVIEW_BATCH: usize = 50;

pub(super) fn api(host: &str, provider: Provider) -> String {
    if provider == Provider::GitHub { "https://api.github.com".into() } else { format!("https://{host}/api/v3") }
}

fn graphql_url(t: &Target) -> String {
    if t.provider == Provider::GitHub { "https://api.github.com/graphql".into() } else { format!("https://{}/api/graphql", t.host) }
}

fn repo_api(t: &Target) -> String {
    format!("{}/repos/{}/{}", api(&t.host, t.provider), enc(&t.owner), enc(&t.repo))
}

#[derive(Deserialize)]
struct User {
    login: String,
    #[serde(default)]
    name: Option<String>,
}

fn me(host: &str, provider: Provider, auth: &Auth) -> Result<User> {
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
struct Login {
    login: String,
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
}

#[derive(Deserialize)]
struct Review {
    /// `user` in REST answers, `author` in GraphQL ones; null for deleted accounts.
    #[serde(alias = "author")]
    user: Option<Login>,
    state: String,
}

fn convert(p: ApiPr, reviews: Vec<Review>) -> PullRequest {
    // Latest review per reviewer wins; comments don't change a vote.
    let mut states: Vec<(String, String)> = Vec::new();
    for r in reviews {
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
    for r in p.requested_reviewers {
        if !states.iter().any(|(u, _)| *u == r.login) {
            states.push((r.login, "requested".into()));
        }
    }
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
        url: p.html_url,
    }
}

/// Each pull request's reviews, asked for in a few GraphQL calls rather than one REST call per
/// pull request. Pull requests whose reviews can't be read are left out (and show no votes).
fn reviews(t: &Target, auth: &Auth, numbers: &[u64]) -> HashMap<u64, Vec<Review>> {
    let mut out = HashMap::new();
    for chunk in numbers.chunks(REVIEW_BATCH) {
        let fields: String = chunk
            .iter()
            .map(|n| format!("p{n}: pullRequest(number: {n}) {{ reviews(first: 100) {{ nodes {{ author {{ login }} state }} }} }} "))
            .collect();
        let query = format!("query($owner: String!, $repo: String!) {{ repository(owner: $owner, name: $repo) {{ {fields}}} }}");
        let body = json!({ "query": query, "variables": { "owner": t.owner, "repo": t.repo } });
        let Ok(answer) = post::<serde_json::Value>(auth, &graphql_url(t), body) else { break };
        let repo = &answer["data"]["repository"];
        for n in chunk {
            if let Ok(r) = serde_json::from_value::<Vec<Review>>(repo[format!("p{n}")]["reviews"]["nodes"].clone()) {
                out.insert(*n, r);
            }
        }
    }
    out
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Deserialize)]
struct ApiIssue {
    number: u64,
    title: String,
    state: String,
    html_url: String,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    assignee: Option<Login>,
    /// Present when the "issue" is actually a pull request.
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

fn to_issue(i: ApiIssue) -> Issue {
    Issue {
        key: i.number.to_string(),
        title: i.title,
        state: i.state,
        kind: i.labels.into_iter().next().map(|l| l.name).unwrap_or_else(|| "Issue".into()),
        assignee: i.assignee.map(|a| a.login),
        url: i.html_url,
    }
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
        let mut reviews = reviews(t, auth, &numbers);
        Ok(prs
            .into_iter()
            .map(|p| {
                let r = reviews.remove(&p.number).unwrap_or_default();
                convert(p, r)
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
        Ok(convert(created, Vec::new()))
    }

    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        let login = me(&t.host, t.provider, auth)?.login;
        let url = format!("{}/issues?state=open&assignee={}&per_page=100&sort=updated", repo_api(t), enc(&login));
        let items: Vec<ApiIssue> = page::by_link(auth, url)?;
        Ok(items.into_iter().filter(|i| i.pull_request.is_none()).map(to_issue).collect())
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
}

#[cfg(test)]
mod tests {
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
        let pr = convert(p, reviews);
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
}

//! Jira Cloud (REST v3) and Jira Data Center / Server (REST v2), as issue trackers.

use super::page::{self, Page};
use super::{Account, Auth, Issue, Provider, Target, enc, get};
use crate::error::{Error, Result};
use serde::Deserialize;

const FIELDS: &str = "summary,status,issuetype,assignee";

fn api(host: &str, provider: Provider) -> String {
    let v = if provider == Provider::JiraCloud { 3 } else { 2 };
    format!("https://{host}/rest/api/{v}")
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct User {
    #[serde(default)]
    display_name: String,
}

pub fn verify(account: &Account, auth: &Auth) -> Result<String> {
    Ok(get::<User>(auth, &format!("{}/myself", api(&account.host, account.provider)))?.display_name)
}

#[derive(Deserialize)]
struct Named {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Fields {
    #[serde(default)]
    summary: String,
    status: Option<Named>,
    issuetype: Option<Named>,
    assignee: Option<User>,
}

#[derive(Deserialize)]
struct ApiIssue {
    key: String,
    fields: Fields,
}

/// Jira Cloud pages with a token; Data Center with a start index and total.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    #[serde(default)]
    issues: Vec<ApiIssue>,
    #[serde(default)]
    next_page_token: Option<String>,
    #[serde(default)]
    start_at: usize,
    #[serde(default)]
    total: Option<usize>,
}

fn to_issue(host: &str, i: ApiIssue) -> Issue {
    Issue {
        url: format!("https://{host}/browse/{}", i.key),
        key: i.key,
        title: i.fields.summary,
        state: i.fields.status.map(|s| s.name).unwrap_or_default(),
        kind: i.fields.issuetype.map(|s| s.name).unwrap_or_default(),
        assignee: i.fields.assignee.map(|a| a.display_name),
    }
}

/// `t.project` optionally limits results to one project key.
pub fn my_issues(t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
    let mut jql = String::from("assignee = currentUser() AND statusCategory != Done");
    if !t.project.trim().is_empty() {
        jql.push_str(&format!(" AND project = \"{}\"", t.project.trim()));
    }
    jql.push_str(" ORDER BY updated DESC");
    // Jira Cloud retired /search in favour of /search/jql.
    let path = if t.provider == Provider::JiraCloud { "search/jql" } else { "search" };
    let first = format!("{}/{path}?jql={}&fields={FIELDS}&maxResults=100", api(&t.host, t.provider), enc(&jql));
    let items = page::collect(first.clone(), |url| {
        let s: Search = get(auth, url)?;
        let read = s.start_at + s.issues.len();
        let next = match (s.next_page_token, s.total) {
            (Some(token), _) => Some(page::with_param(&first, "nextPageToken", &token)),
            (None, Some(total)) if read < total => Some(page::with_param(&first, "startAt", &read.to_string())),
            _ => None,
        };
        Ok(Page { items: s.issues, next })
    })?;
    Ok(items.into_iter().map(|i| to_issue(&t.host, i)).collect())
}

/// Looks keys up one by one: a JQL `key in (...)` fails entirely if any key doesn't exist.
pub fn issues(t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
    let mut out = Vec::new();
    for k in keys.iter().take(30) {
        match get::<ApiIssue>(auth, &format!("{}/issue/{}?fields={FIELDS}", api(&t.host, t.provider), enc(k))) {
            Ok(i) => out.push(to_issue(&t.host, i)),
            Err(e) if e.is_not_found() => {}
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// The Jira [`Forge`](super::Forge): an issue tracker only.
pub struct Jira;

impl super::Forge for Jira {
    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        verify(account, auth)
    }
    fn pull_requests(&self, _t: &Target, _auth: &Auth) -> Result<Vec<super::PullRequest>> {
        Ok(Vec::new())
    }
    fn default_branch(&self, _t: &Target, _auth: &Auth) -> Result<Option<String>> {
        Ok(None)
    }
    fn create_pull_request(&self, _t: &Target, _auth: &Auth, _pr: &super::NewPullRequest) -> Result<super::PullRequest> {
        Err(Error::Unsupported("Jira does not host pull requests".into()))
    }
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        my_issues(t, auth)
    }
    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        issues(t, auth, keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_search_results() {
        let s: Search = serde_json::from_str(
            r#"{"issues":[{"id":"1","key":"PROJ-12","fields":{"summary":"Crash","status":{"name":"In Progress"},
            "issuetype":{"name":"Bug"},"assignee":{"displayName":"Ada"}}}],"nextPageToken":null}"#,
        )
        .unwrap();
        let i = to_issue("acme.atlassian.net", s.issues.into_iter().next().unwrap());
        assert_eq!((i.key.as_str(), i.state.as_str(), i.kind.as_str()), ("PROJ-12", "In Progress", "Bug"));
        assert_eq!(i.url, "https://acme.atlassian.net/browse/PROJ-12");
        assert_eq!(api("jira.acme.io", Provider::JiraDataCenter), "https://jira.acme.io/rest/api/2");
    }
}

//! Jira Cloud (REST v3) and Jira Data Center / Server (REST v2), as issue trackers.

use super::page::{self, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, Conditions, CreateOptions, Entry, FilterKind, FilterOptions, Issue, IssueDetails, IssueFacets, Me, NewIssue,
    NewPullRequest, Provider, PullRequest, RefStyle, Target, enc, get, jira_issues,
};
use crate::error::{Error, Result};
use serde::Deserialize;

pub(super) static JIRA_CLOUD: ProviderInfo = ProviderInfo {
    name: "Jira Cloud",
    host_hint: "e.g., mycompany.atlassian.net",
    username_label: Some("Email"),
    token_help: "Your Atlassian account email and an API token.",
    ..JIRA_DATA_CENTER
};

pub(super) static JIRA_DATA_CENTER: ProviderInfo = ProviderInfo {
    name: "Jira Data Center",
    host_hint: "e.g., jira.mycompany.com",
    token_help: "A personal access token.",
    ref_style: RefStyle::Jira,
    is_jira: true,
    tracker_only: true,
    issues_in_app: true,
    service: &Jira,
    ..DEFAULTS
};

pub(super) const FIELDS: &str = "summary,status,issuetype,assignee,labels,fixVersions";

pub(super) fn api(host: &str, provider: Provider) -> String {
    let v = if provider == Provider::JiraCloud { 3 } else { 2 };
    format!("https://{host}/rest/api/{v}")
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct User {
    #[serde(default)]
    display_name: String,
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
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default, rename = "fixVersions")]
    fix_versions: Vec<Named>,
}

#[derive(Deserialize)]
pub(super) struct ApiIssue {
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

#[derive(Deserialize)]
struct Key {
    key: String,
}

/// A page of Jira Cloud's project search.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPage {
    #[serde(default)]
    values: Vec<Key>,
    #[serde(default)]
    is_last: Option<bool>,
    #[serde(default)]
    start_at: usize,
}

pub(super) fn to_issue(host: &str, i: ApiIssue) -> Issue {
    let milestone = i.fields.fix_versions.into_iter().next().map(|v| v.name);
    Issue {
        url: format!("https://{host}/browse/{}", i.key),
        key: i.key,
        title: i.fields.summary,
        state: i.fields.status.map(|s| s.name).unwrap_or_default(),
        kind: i.fields.issuetype.map(|s| s.name).unwrap_or_default(),
        assignee: i.fields.assignee.map(|a| a.display_name),
        facets: IssueFacets { labels: i.fields.labels, milestone },
    }
}

/// Every issue the JQL query finds. Jira Cloud pages with a token, Data Center with a start index.
pub(super) fn search(t: &Target, auth: &Auth, jql: &str) -> Result<Vec<Issue>> {
    // Jira Cloud retired /search in favour of /search/jql.
    let path = if t.provider == Provider::JiraCloud { "search/jql" } else { "search" };
    let first = format!("{}/{path}?jql={}&fields={FIELDS}&maxResults=100", api(&t.host, t.provider), enc(jql));
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

/// The Jira [`Forge`](super::Forge): an issue tracker only.
pub struct Jira;

impl super::Forge for Jira {
    fn create_pull_request(&self, _t: &Target, _auth: &Auth, _pr: &NewPullRequest) -> Result<PullRequest> {
        Err(Error::Unsupported("Jira does not host pull requests".into()))
    }

    fn issue_url(&self, t: &Target, key: &str) -> String {
        format!("https://{}/browse/{}", t.host, enc(key))
    }

    fn token_url(&self, account: &Account) -> Option<String> {
        Some(match account.provider {
            Provider::JiraCloud => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
            _ => format!("https://{}/secure/ViewProfile.jspa", account.host),
        })
    }

    fn display(&self, t: &Target) -> String {
        if t.project.is_empty() { t.host.clone() } else { format!("{} · {}", t.host, t.project) }
    }

    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        Ok(get::<User>(auth, &format!("{}/myself", api(&account.host, account.provider)))?.display_name)
    }

    /// `t.project` optionally limits results to one project key.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        search(t, auth, &super::filters::jira::jql(&super::Builtin::IssuesAssignedToMe.conditions(), &t.project))
    }

    fn find_issues(&self, t: &Target, auth: &Auth, c: &Conditions, _me: Option<&Me>) -> Result<Vec<Issue>> {
        super::filters::jira::find_issues(t, auth, c)
    }

    fn filter_options(&self, t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
        super::filters::jira::options(t, auth, kind)
    }

    /// Cloud pages through the project search; Data Center lists every project at once. A list
    /// cut off at `MAX_ITEMS` is None: an incomplete list would reject real keys.
    fn project_keys(&self, t: &Target, auth: &Auth) -> Result<Option<Vec<String>>> {
        let api = api(&t.host, t.provider);
        if t.provider != Provider::JiraCloud {
            return Ok(Some(get::<Vec<Key>>(auth, &format!("{api}/project"))?.into_iter().map(|k| k.key).collect()));
        }
        let first = format!("{api}/project/search?maxResults=100");
        let mut more = false;
        let keys = page::collect(first.clone(), |url| {
            let p: ProjectPage = get(auth, url)?;
            more = p.is_last == Some(false) && !p.values.is_empty();
            let next = more.then(|| page::with_param(&first, "startAt", &(p.start_at + p.values.len()).to_string()));
            Ok(Page { items: p.values, next })
        })?;
        Ok((!more).then(|| keys.into_iter().map(|k| k.key).collect()))
    }

    fn issue_details(&self, t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
        jira_issues::details(t, auth, key)
    }

    fn issue_create_options(&self, t: &Target, auth: &Auth, project: &str) -> Result<CreateOptions> {
        jira_issues::options(t, auth, project)
    }

    fn create_issue(&self, t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
        jira_issues::create(t, auth, new)
    }

    fn post_issue_comment(&self, t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
        jira_issues::post_comment(t, auth, key, body)
    }

    /// Looks keys up one by one: a JQL `key in (...)` fails entirely if any key doesn't exist.
    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
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

    #[test]
    fn issue_facets_from_labels_and_fix_versions() {
        let i: ApiIssue = serde_json::from_str(
            r#"{"key":"P-1","fields":{"summary":"S","labels":["ui","perf"],"fixVersions":[{"name":"2.0"},{"name":"2.1"}]}}"#,
        )
        .unwrap();
        assert_eq!(to_issue("h", i).facets, IssueFacets { labels: vec!["ui".into(), "perf".into()], milestone: Some("2.0".into()) });
    }
}

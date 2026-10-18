//! Issues on GitHub and GitHub Enterprise Server (REST API v3): the listed issue model, one
//! issue with its comments, the Create issue dialog's labels and assignees, and creating an
//! issue.

use super::filters::github::labels;
use super::github::{Label, Login, Title, repo_api};
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, missing};
use super::{Auth, Entry, EntryKind, Issue, IssueFacets, Target, get, page, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// How a deleted account is shown.
const GHOST: &str = "ghost";

#[derive(Deserialize)]
pub(super) struct ApiIssue {
    number: u64,
    title: String,
    state: String,
    html_url: String,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    assignee: Option<Login>,
    #[serde(default)]
    milestone: Option<Title>,
    /// Present when the "issue" is actually a pull request.
    #[serde(default)]
    pub(super) pull_request: Option<serde_json::Value>,
}

pub(super) fn to_issue(i: ApiIssue) -> Issue {
    let labels: Vec<String> = i.labels.into_iter().map(|l| l.name).collect();
    Issue {
        key: i.number.to_string(),
        title: i.title,
        state: i.state,
        kind: labels.first().cloned().unwrap_or_else(|| "Issue".into()),
        assignee: i.assignee.map(|a| a.login),
        url: i.html_url,
        facets: IssueFacets { labels, milestone: i.milestone.map(|m| m.title) },
    }
}

/// An issue as its own page reads it.
#[derive(Deserialize)]
struct ApiIssueFull {
    #[serde(flatten)]
    base: ApiIssue,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    assignees: Vec<Login>,
    #[serde(default)]
    state_reason: Option<String>,
}

#[derive(Deserialize)]
struct ApiComment {
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    html_url: String,
}

fn author(user: Option<Login>) -> String {
    user.map_or_else(|| GHOST.to_owned(), |u| u.login)
}

/// Issue `key` and its comments, oldest first. A pull request (which GitHub also answers for
/// as an issue) is refused.
pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    let Ok(n) = key.parse::<u64>() else { return Err(Error::NotFound(format!("Issue #{key} wasn't found"))) };
    let full: ApiIssueFull = get(auth, &format!("{}/issues/{n}", repo_api(t))).map_err(missing(t, key))?;
    if full.base.pull_request.is_some() {
        return Err(Error::NotFound(format!("#{n} is a pull request")));
    }
    let comments: Vec<ApiComment> = page::by_link(auth, format!("{}/issues/{n}/comments?per_page=100", repo_api(t)))?;
    let not_planned = full.state_reason.as_deref() == Some("not_planned");
    let mut issue = to_issue(full.base);
    if not_planned && issue.state == "closed" {
        issue.state = "closed (not planned)".into();
    }
    let comments = comments
        .into_iter()
        .map(|c| Entry {
            author: author(c.user),
            time: api_time(&c.created_at),
            kind: EntryKind::Comment,
            body: c.body.unwrap_or_default(),
            url: c.html_url,
        })
        .collect();
    Ok(IssueDetails {
        issue,
        body: full.body.unwrap_or_default(),
        author: author(full.user),
        created: api_time(&full.created_at),
        assignees: full.assignees.into_iter().map(|a| a.login).collect(),
        priority: None,
        epic: None,
        comments,
    })
}

/// The repository's labels and the people issues can be assigned to.
pub(super) fn options(t: &Target, auth: &Auth) -> Result<CreateOptions> {
    let labels = labels(t, auth)?;
    let people: Vec<Login> = page::by_link(auth, format!("{}/assignees?per_page=100", repo_api(t)))?;
    Ok(CreateOptions {
        labels,
        people: people.into_iter().map(|p| Person { id: p.login.clone(), name: p.login }).collect(),
        labels_free: false,
        max_assignees: 10,
        ..CreateOptions::default()
    })
}

/// The JSON body of a new issue: empty lists and an empty body are left out.
fn body(new: &NewIssue) -> Value {
    let mut m = Map::new();
    m.insert("title".into(), json!(new.title));
    if !new.description.is_empty() {
        m.insert("body".into(), json!(new.description));
    }
    if !new.labels.is_empty() {
        m.insert("labels".into(), json!(new.labels));
    }
    if !new.assignees.is_empty() {
        m.insert("assignees".into(), json!(new.assignees));
    }
    Value::Object(m)
}

pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    let created: ApiIssue = post(auth, &format!("{}/issues", repo_api(t)), body(new))?;
    Ok(to_issue(created))
}

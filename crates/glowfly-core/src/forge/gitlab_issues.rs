//! Issues on GitLab.com and GitLab Self-Managed (REST API v4): one issue with its comments
//! (notes; system notes are left out), the Create issue dialog's labels, members and types,
//! creating an issue and posting a comment.

use super::filters::Named;
use super::filters::gitlab::labels;
use super::gitlab::{ApiIssue, User, project_api, to_issue};
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, missing, not_found};
use super::{Auth, Entry, EntryKind, Issue, Target, get, links, page, post};
use crate::error::Result;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
struct ApiIssueFull {
    #[serde(flatten)]
    base: ApiIssue,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<User>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    assignees: Vec<User>,
}

#[derive(Deserialize)]
struct ApiNote {
    id: u64,
    #[serde(default)]
    body: String,
    #[serde(default)]
    author: Option<User>,
    #[serde(default)]
    created_at: String,
    /// Written by GitLab itself ("changed the description", "added label …").
    #[serde(default)]
    system: bool,
}

#[derive(Deserialize)]
struct Member {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    username: String,
}

/// A user's name, else their username.
fn shown(u: Option<User>) -> String {
    u.map(|u| if u.name.is_empty() { u.username } else { u.name }).unwrap_or_default()
}

/// A note as a comment; `page_url` is the issue's checked page (the API sends no note links,
/// so a note's page is the issue's with an anchor).
fn note_entry(n: ApiNote, page_url: &str) -> Entry {
    Entry {
        author: shown(n.author),
        time: api_time(&n.created_at),
        kind: EntryKind::Comment,
        body: n.body,
        url: if page_url.is_empty() { String::new() } else { format!("{page_url}#note_{}", n.id) },
    }
}

pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    let Ok(iid) = key.parse::<u64>() else { return Err(not_found(t, key)) };
    let full: ApiIssueFull = get(auth, &format!("{}/issues/{iid}", project_api(t))).map_err(missing(t, key))?;
    let url = format!("{}/issues/{iid}/notes?sort=asc&order_by=created_at&per_page=100", project_api(t));
    let notes: Vec<ApiNote> = page::by_link(auth, url)?;
    let issue = to_issue(full.base);
    let page_url = links::web_url(t, issue.url.clone());
    let comments = notes.into_iter().filter(|n| !n.system).map(|n| note_entry(n, &page_url)).collect();
    Ok(IssueDetails {
        issue,
        body: full.description.unwrap_or_default(),
        author: shown(full.author),
        created: api_time(&full.created_at),
        assignees: full.assignees.into_iter().map(|a| shown(Some(a))).collect(),
        priority: None,
        epic: None,
        comments,
    })
}

/// The project's labels and members (one assignee: more is a paid feature), and the two
/// issue types every tier has.
pub(super) fn options(t: &Target, auth: &Auth) -> Result<CreateOptions> {
    let labels = labels(t, auth)?;
    let members: Vec<Member> = page::by_link(auth, format!("{}/members/all?per_page=100", project_api(t)))?;
    let mut people: Vec<Person> = Vec::new();
    for m in members {
        let id = m.id.to_string();
        if !people.iter().any(|p| p.id == id) {
            people.push(Person { id, name: if m.name.is_empty() { m.username } else { m.name } });
        }
    }
    let types = [("issue", "Issue"), ("incident", "Incident")].map(|(key, title)| Named { key: key.into(), title: title.into() });
    Ok(CreateOptions { labels, people, types: types.to_vec(), labels_free: true, max_assignees: 1, ..Default::default() })
}

fn body(new: &NewIssue) -> Value {
    let mut m = Map::new();
    m.insert("title".into(), json!(new.title));
    if !new.description.is_empty() {
        m.insert("description".into(), json!(new.description));
    }
    if !new.labels.is_empty() {
        m.insert("labels".into(), json!(new.labels.join(",")));
    }
    let ids: Vec<u64> = new.assignees.iter().filter_map(|a| a.parse().ok()).collect();
    if !ids.is_empty() {
        m.insert("assignee_ids".into(), json!(ids));
    }
    if let Some(k) = &new.kind {
        m.insert("issue_type".into(), json!(k));
    }
    Value::Object(m)
}

pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    let created: ApiIssue = post(auth, &format!("{}/issues", project_api(t)), body(new))?;
    Ok(to_issue(created))
}

/// Posts `body` as a note on issue `key`.
pub(super) fn post_comment(t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
    let Ok(iid) = key.parse::<u64>() else { return Err(not_found(t, key)) };
    let n: ApiNote = post(auth, &format!("{}/issues/{iid}/notes", project_api(t)), json!({ "body": body })).map_err(missing(t, key))?;
    Ok(note_entry(n, &links::web_url(t, t.issue_url(key))))
}

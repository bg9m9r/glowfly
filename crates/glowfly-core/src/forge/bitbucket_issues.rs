//! Bitbucket Cloud issues in the issue panel and the Create dialog: one issue with its raw
//! (Markdown) description, priority and comments; the kinds, priorities and workspace members
//! to choose from; creating an issue; and posting a comment. A repository whose issue tracker is
//! turned off says so in one sentence.

use super::bitbucket::{CLOUD, CloudIssue, CloudUser, Links, cloud_list, cloud_repo, to_issue};
use super::filters::Named;
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, missing, not_found};
use super::{Auth, Entry, EntryKind, Issue, Target, enc, get, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

const TRACKER_OFF: &str = "This repository's issue tracker is turned off on Bitbucket. Turn it on in the repository's settings on \
                           Bitbucket to create and view issues.";

const KINDS: [&str; 4] = ["bug", "enhancement", "proposal", "task"];
const PRIORITIES: [&str; 5] = ["trivial", "minor", "major", "critical", "blocker"];

/// Bitbucket's 404 for a repository without an issue tracker, in words that say what to do.
fn tracker_off(e: Error) -> Error {
    if e.is_not_found() && e.to_string().to_lowercase().contains("issue tracker") { Error::Unsupported(TRACKER_OFF.into()) } else { e }
}

#[derive(Deserialize, Default)]
struct Content {
    #[serde(default)]
    raw: Option<String>,
}

/// An issue as its own page reads it.
#[derive(Deserialize)]
struct Full {
    #[serde(flatten)]
    base: CloudIssue,
    #[serde(default)]
    content: Option<Content>,
    #[serde(default)]
    reporter: Option<CloudUser>,
    #[serde(default)]
    created_on: String,
    #[serde(default)]
    priority: Option<String>,
}

#[derive(Deserialize)]
struct Comment {
    #[serde(default)]
    content: Option<Content>,
    #[serde(default)]
    user: Option<CloudUser>,
    #[serde(default)]
    created_on: String,
    #[serde(default)]
    links: Option<Links>,
}

fn raw(c: Option<Content>) -> String {
    c.and_then(|c| c.raw).unwrap_or_default()
}

fn comment_entry(c: Comment) -> Entry {
    Entry {
        author: c.user.map(|u| u.display_name).unwrap_or_default(),
        time: api_time(&c.created_on),
        kind: EntryKind::Comment,
        body: raw(c.content),
        url: c.links.map(|l| l.html.href).unwrap_or_default(),
    }
}

/// Issue `key` with its description and comments, oldest first; comments that only changed
/// the issue (no text) are left out.
pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    let Ok(n) = key.parse::<u64>() else { return Err(not_found(t, key)) };
    let full: Full = get(auth, &format!("{}/issues/{n}", cloud_repo(t))).map_err(tracker_off).map_err(missing(t, key))?;
    let comments: Vec<Comment> = cloud_list(auth, format!("{}/issues/{n}/comments?sort=created_on&pagelen=100", cloud_repo(t)))?;
    let comments = comments.into_iter().map(comment_entry).filter(|c| !c.body.trim().is_empty()).collect();
    let issue = to_issue(full.base);
    Ok(IssueDetails {
        assignees: issue.assignee.iter().filter(|a| !a.is_empty()).cloned().collect(),
        issue,
        body: raw(full.content),
        author: full.reporter.map(|u| u.display_name).unwrap_or_default(),
        created: api_time(&full.created_on),
        priority: full.priority.filter(|p| !p.is_empty()),
        epic: None,
        comments,
    })
}

fn named(keys: &[&str]) -> Vec<Named> {
    keys.iter()
        .map(|k| {
            let mut c = k.chars();
            let title = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
            Named { key: (*k).to_owned(), title }
        })
        .collect()
}

/// The kinds and priorities every Bitbucket tracker has, and the workspace's members (none
/// when the app password may not read them). A repository with the tracker turned off says so.
pub(super) fn options(t: &Target, auth: &Auth) -> Result<CreateOptions> {
    #[derive(Deserialize)]
    struct Repo {
        #[serde(default)]
        has_issues: Option<bool>,
    }
    #[derive(Deserialize)]
    struct Member {
        user: CloudUser,
    }
    let repo: Repo = get(auth, &cloud_repo(t))?;
    if repo.has_issues == Some(false) {
        return Err(Error::Unsupported(TRACKER_OFF.into()));
    }
    let members = match cloud_list::<Member>(auth, format!("{CLOUD}/workspaces/{}/members?pagelen=100", enc(&t.owner))) {
        Err(Error::Forbidden(_)) => Vec::new(),
        other => other?,
    };
    let mut people: Vec<Person> = Vec::new();
    for u in members.into_iter().map(|m| m.user).filter(|u| !u.uuid.is_empty()) {
        if !people.iter().any(|p| p.id == u.uuid) {
            let name = if u.display_name.is_empty() { u.uuid.clone() } else { u.display_name };
            people.push(Person { id: u.uuid, name });
        }
    }
    Ok(CreateOptions {
        people,
        types: named(&KINDS),
        priorities: named(&PRIORITIES),
        default_type: Some("bug".into()),
        default_priority: Some("major".into()),
        labels_free: false,
        max_assignees: 1,
        ..CreateOptions::default()
    })
}

/// The JSON body of a new issue: an empty description and unset fields are left out.
fn body(new: &NewIssue) -> Value {
    let mut m = Map::new();
    m.insert("title".into(), json!(new.title));
    if !new.description.is_empty() {
        m.insert("content".into(), json!({ "raw": new.description }));
    }
    if let Some(k) = new.kind.as_deref().filter(|k| !k.is_empty()) {
        m.insert("kind".into(), json!(k));
    }
    if let Some(p) = new.priority.as_deref().filter(|p| !p.is_empty()) {
        m.insert("priority".into(), json!(p));
    }
    if let Some(a) = new.assignees.first() {
        m.insert("assignee".into(), json!({ "uuid": a }));
    }
    Value::Object(m)
}

pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    let created: CloudIssue = post(auth, &format!("{}/issues", cloud_repo(t)), body(new)).map_err(tracker_off)?;
    Ok(to_issue(created))
}

/// Posts `body` as a comment on issue `key`.
pub(super) fn post_comment(t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
    let Ok(n) = key.parse::<u64>() else { return Err(not_found(t, key)) };
    let url = format!("{}/issues/{n}/comments", cloud_repo(t));
    let c: Comment = post(auth, &url, json!({ "content": { "raw": body } })).map_err(tracker_off).map_err(missing(t, key))?;
    Ok(comment_entry(c))
}

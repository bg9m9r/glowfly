//! Issues on Jira Cloud (REST v3, rich text as ADF) and Jira Data Center (REST v2, plain
//! strings): one issue with its comments and epic, the Create issue dialog's issue types,
//! assignable people, epics and labels, and creating an issue. Cloud links an epic as the
//! issue's `parent`; Data Center through its Epic Link custom field, found by its schema.

use super::filters::{Named, esc_quoted};
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, missing};
use super::jira::{ApiIssue, FIELDS, api, search, to_issue};
use super::page::{self, Page};
use super::{Auth, Entry, EntryKind, Forge, Issue, Provider, Target, enc, get, jira_adf, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// The schema of Data Center's Epic Link field.
const EPIC_LINK: &str = "com.pyxis.greenhopper.jira:gh-epic-link";

fn cloud(t: &Target) -> bool {
    t.provider == Provider::JiraCloud
}

/// Data Center's Epic Link field id (`customfield_10008`, say); None on Cloud, which uses
/// `parent`, and on a site without the field.
fn epic_field(t: &Target, auth: &Auth) -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct Schema {
        #[serde(default)]
        custom: Option<String>,
    }
    #[derive(Deserialize)]
    struct Field {
        id: String,
        #[serde(default)]
        schema: Option<Schema>,
    }
    if cloud(t) {
        return Ok(None);
    }
    let fields: Vec<Field> = get(auth, &format!("{}/field", api(&t.host, t.provider)))?;
    Ok(fields.into_iter().find(|f| f.schema.as_ref().and_then(|s| s.custom.as_deref()) == Some(EPIC_LINK)).map(|f| f.id))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentPage {
    #[serde(default)]
    comments: Vec<Value>,
    #[serde(default)]
    start_at: usize,
    #[serde(default)]
    total: Option<usize>,
}

fn name_of(v: &Value) -> String {
    v["displayName"].as_str().unwrap_or_default().to_owned()
}

/// Issue `key` with its description, reporter, epic and comments, oldest first.
pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    let base = api(&t.host, t.provider);
    // Without the field list (a token that can't read it) the issue still loads, without its epic.
    let epic_field = epic_field(t, auth).unwrap_or(None);
    let extra = epic_field.as_ref().map(|f| format!(",{f}")).unwrap_or_default();
    let url = format!("{base}/issue/{}?fields={FIELDS},description,reporter,created,parent{extra}", enc(key));
    let v: Value = get(auth, &url).map_err(missing(t, key))?;
    let fields = &v["fields"];
    let issue = to_issue(&t.host, serde_json::from_value::<ApiIssue>(v.clone())?);
    let epic = match &epic_field {
        Some(f) => fields[f.as_str()].as_str().filter(|k| !k.is_empty()).map(str::to_owned),
        None => {
            let p = &fields["parent"];
            let is_epic = p["fields"]["issuetype"]["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case("epic"));
            let summary = p["fields"]["summary"].as_str().unwrap_or_default();
            p["key"].as_str().filter(|_| is_epic).map(|k| format!("{k}  {summary}").trim_end().to_owned())
        }
    };
    let first = format!("{base}/issue/{}/comment?orderBy=created&maxResults=100", enc(key));
    let comments = page::collect(first.clone(), |url| {
        let p: CommentPage = get(auth, url)?;
        let read = p.start_at + p.comments.len();
        let next = p.total.filter(|total| read < *total).map(|_| page::with_param(&first, "startAt", &read.to_string()));
        Ok(Page { items: p.comments, next })
    })?;
    let comments = comments
        .iter()
        .map(|c| Entry {
            author: name_of(&c["author"]),
            time: api_time(c["created"].as_str().unwrap_or_default()),
            kind: EntryKind::Comment,
            body: jira_adf::to_text(&c["body"]),
            url: String::new(),
        })
        .collect();
    let assignee = name_of(&fields["assignee"]);
    Ok(IssueDetails {
        issue,
        body: jira_adf::to_text(&fields["description"]),
        author: name_of(&fields["reporter"]),
        created: api_time(fields["created"].as_str().unwrap_or_default()),
        assignees: if assignee.is_empty() { Vec::new() } else { vec![assignee] },
        epic,
        comments,
    })
}

/// The project's issue types (without subtasks), the people assignable in it, its open epics
/// and (Cloud) the site's labels.
pub(super) fn options(t: &Target, auth: &Auth, project: &str) -> Result<CreateOptions> {
    #[derive(Deserialize)]
    struct IssueType {
        id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        subtask: bool,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Project {
        #[serde(default)]
        issue_types: Vec<IssueType>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct User {
        #[serde(default)]
        account_id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        display_name: String,
    }
    let project = project.trim();
    if project.is_empty() {
        return Err(Error::Other("Choose a project.".into()));
    }
    let base = api(&t.host, t.provider);
    let p: Project = get(auth, &format!("{base}/project/{}", enc(project)))?;
    let types = p.issue_types.into_iter().filter(|i| !i.subtask).map(|i| Named { key: i.id, title: i.name }).collect();
    let users: Vec<User> = get(auth, &format!("{base}/user/assignable/search?project={}&maxResults=100", enc(project)))?;
    let people = users
        .into_iter()
        .filter_map(|u| {
            let id = if cloud(t) { u.account_id } else { u.name }?;
            let name = if u.display_name.is_empty() { id.clone() } else { u.display_name };
            Some(Person { id, name })
        })
        .collect();
    let jql = format!("project = \"{}\" AND issuetype = Epic AND statusCategory != Done ORDER BY updated DESC", esc_quoted(project, '"'));
    // A project without the Epic type refuses the query: no epics to offer.
    let epics = search(t, auth, &jql).map(|found| found.into_iter().map(|i| Named { key: i.key, title: i.title }).collect());
    let labels = if cloud(t) { super::filters::jira::cloud_labels(t, auth)? } else { Vec::new() };
    Ok(CreateOptions { labels, people, types, epics: epics.unwrap_or_default(), labels_free: true, max_assignees: 1 })
}

/// The `fields` of a new issue: Cloud takes rich text and account ids and links the epic as
/// the parent; Data Center takes plain text, usernames and its Epic Link field.
fn fields(t: &Target, new: &NewIssue, epic_field: Option<&str>) -> Result<Value> {
    let mut f = Map::new();
    f.insert("project".into(), json!({ "key": new.project.trim() }));
    f.insert("summary".into(), json!(new.title));
    if let Some(k) = &new.kind {
        f.insert("issuetype".into(), json!({ "id": k }));
    }
    if !new.description.trim().is_empty() {
        let d = if cloud(t) { jira_adf::from_text(&new.description) } else { json!(new.description) };
        f.insert("description".into(), d);
    }
    if !new.labels.is_empty() {
        f.insert("labels".into(), json!(new.labels));
    }
    if let Some(a) = new.assignees.first() {
        f.insert("assignee".into(), if cloud(t) { json!({ "accountId": a }) } else { json!({ "name": a }) });
    }
    if let Some(e) = &new.epic {
        match (cloud(t), epic_field) {
            (true, _) => {
                f.insert("parent".into(), json!({ "key": e }));
            }
            (false, Some(field)) => {
                f.insert(field.into(), json!(e));
            }
            (false, None) => return Err(Error::Other("This Jira site has no Epic Link field, so the epic can't be set.".into())),
        }
    }
    Ok(Value::Object(f))
}

/// Creates the issue, then reads it back for its status and type.
pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    #[derive(Deserialize)]
    struct Created {
        key: String,
    }
    let epic_field = if new.epic.is_some() { epic_field(t, auth)? } else { None };
    let body = json!({ "fields": fields(t, new, epic_field.as_deref())? });
    let created: Created = post(auth, &format!("{}/issue", api(&t.host, t.provider)), body)?;
    let read = super::jira::Jira.issues(t, auth, std::slice::from_ref(&created.key)).unwrap_or_default();
    Ok(read.into_iter().next().unwrap_or_else(|| Issue {
        url: super::jira::Jira.issue_url(t, &created.key),
        key: created.key,
        title: new.title.clone(),
        state: String::new(),
        kind: String::new(),
        assignee: None,
        facets: Default::default(),
    }))
}

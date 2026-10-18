//! Azure Boards work items in the issue panel and the Create dialog: one work item with its
//! description (HTML, shown as text), parent and comments; the project's work item types,
//! tags, default team members and open epics and features to file a work item under; and
//! creating a work item with a JSON Patch.

use super::azure::{List, Name, V, project_base};
use super::azure_work_items::{Fields, WorkItem, to_issue, wiql, work_items};
use super::filters::Named;
use super::html_text::{from_text, to_text};
use super::http::post_json_patch;
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, missing, not_found};
use super::page::{self, Page};
use super::{Auth, Entry, EntryKind, Issue, Target, enc, get};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

/// The link from a work item to its parent.
const PARENT: &str = "System.LinkTypes.Hierarchy-Reverse";
/// The comments API's version (still a preview in 7.1).
const COMMENTS_V: &str = "7.1-preview.4";

#[derive(Deserialize)]
struct Relation {
    #[serde(default)]
    rel: String,
    #[serde(default)]
    url: String,
}

/// A work item read with its relations; the fields stay raw for the HTML and identity ones.
#[derive(Deserialize)]
struct Full {
    id: u32,
    #[serde(default)]
    fields: Value,
    #[serde(default)]
    relations: Vec<Relation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentPage {
    #[serde(default)]
    comments: Vec<Value>,
    #[serde(default)]
    continuation_token: Option<String>,
}

fn display_name(v: &Value) -> String {
    v["displayName"].as_str().unwrap_or_default().to_owned()
}

fn text_of(fields: &Value, name: &str) -> String {
    to_text(fields[name].as_str().unwrap_or_default())
}

/// Work item `key` with its description, people, parent and comments, oldest first.
pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    let Ok(id) = key.parse::<u32>() else { return Err(not_found(t, key)) };
    let url = format!("{}/_apis/wit/workitems/{id}?$expand=relations&api-version={V}", project_base(t));
    let full: Full = get(auth, &url).map_err(missing(t, key))?;
    let f = &full.fields;
    let fields: Fields = serde_json::from_value(f.clone()).unwrap_or_default();
    let issue = to_issue(t, WorkItem { id: full.id, fields });
    let mut body = text_of(f, "System.Description");
    if body.is_empty() && issue.kind.eq_ignore_ascii_case("bug") {
        body = text_of(f, "Microsoft.VSTS.TCM.ReproSteps");
    }
    let assignee = display_name(&f["System.AssignedTo"]);
    let parent =
        full.relations.iter().find(|r| r.rel == PARENT).and_then(|r| r.url.rsplit('/').next()).filter(|i| i.parse::<u32>().is_ok());
    let epic = parent.map(|p| match work_items(t, auth, &[p.to_owned()]) {
        Ok(found) => found.into_iter().next().map_or_else(|| p.to_owned(), |w| format!("{p}  {}", w.title).trim_end().to_owned()),
        Err(_) => p.to_owned(),
    });
    let first = format!("{}/_apis/wit/workItems/{id}/comments?$top=200&order=asc&api-version={COMMENTS_V}", project_base(t));
    let comments = page::collect(first.clone(), |url| {
        let p: CommentPage = get(auth, url)?;
        let next = p.continuation_token.filter(|c| !c.is_empty()).map(|c| page::with_param(&first, "continuationToken", &c));
        Ok(Page { items: p.comments, next })
    })?;
    let comments = comments
        .iter()
        .filter(|c| !c["isDeleted"].as_bool().unwrap_or(false))
        .map(|c| Entry {
            author: display_name(&c["createdBy"]),
            time: api_time(c["createdDate"].as_str().unwrap_or_default()),
            kind: EntryKind::Comment,
            body: to_text(c["text"].as_str().unwrap_or_default()),
            url: String::new(),
        })
        .collect();
    Ok(IssueDetails {
        issue,
        body,
        author: display_name(&f["System.CreatedBy"]),
        created: api_time(f["System.CreatedDate"].as_str().unwrap_or_default()),
        assignees: if assignee.is_empty() { Vec::new() } else { vec![assignee] },
        priority: None,
        epic,
        comments,
    })
}

/// The project's work item types (without disabled and hidden ones) with its requirement type
/// first chosen, its tags, its default team's members, and the open epics and features to file
/// a work item under.
pub(super) fn options(t: &Target, auth: &Auth) -> Result<CreateOptions> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Type {
        name: String,
        #[serde(default)]
        is_disabled: bool,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Category {
        #[serde(default)]
        reference_name: String,
        #[serde(default)]
        default_work_item_type: Option<Name>,
        #[serde(default)]
        work_item_types: Vec<Name>,
    }
    let base = project_base(t);
    let all: List<Type> = get(auth, &format!("{base}/_apis/wit/workitemtypes?api-version={V}"))?;
    let categories: List<Category> = get(auth, &format!("{base}/_apis/wit/workitemtypecategories?api-version={V}"))?;
    let hidden: Vec<&str> = categories
        .value
        .iter()
        .filter(|c| c.reference_name == "Microsoft.HiddenCategory")
        .flat_map(|c| c.work_item_types.iter().map(|n| n.name.as_str()))
        .collect();
    let types: Vec<Named> = all
        .value
        .iter()
        .filter(|w| !w.is_disabled && !hidden.contains(&w.name.as_str()))
        .map(|w| Named { key: w.name.clone(), title: w.name.clone() })
        .collect();
    let default_type = categories
        .value
        .iter()
        .find(|c| c.reference_name == "Microsoft.RequirementCategory")
        .and_then(|c| c.default_work_item_type.as_ref())
        .map(|n| n.name.clone())
        .filter(|n| types.iter().any(|w| &w.key == n));
    let labels = super::filters::azure::tag_names(t, auth)?;
    let people = match team_members(t, auth) {
        Err(Error::Forbidden(_)) => Vec::new(),
        other => other?,
    };
    let open = "[System.TeamProject] = @project AND [System.WorkItemType] IN ('Epic', 'Feature') \
                AND [System.State] NOT IN ('Closed', 'Done', 'Removed', 'Resolved')";
    let epics = wiql(t, auth, open)?.into_iter().map(|i| Named { title: format!("{} · {}", i.kind, i.title), key: i.key }).collect();
    Ok(CreateOptions { labels, people, types, default_type, epics, labels_free: true, max_assignees: 1, ..CreateOptions::default() })
}

/// The members of the project's default team, by uniqueName (groups left out).
fn team_members(t: &Target, auth: &Auth) -> Result<Vec<Person>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Project {
        #[serde(default)]
        default_team: Option<Team>,
    }
    #[derive(Deserialize)]
    struct Team {
        id: String,
    }
    #[derive(Deserialize)]
    struct Member {
        identity: Identity,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Identity {
        #[serde(default)]
        display_name: String,
        #[serde(default)]
        unique_name: String,
        #[serde(default)]
        is_container: bool,
    }
    let project = format!("https://dev.azure.com/{}/_apis/projects/{}", enc(&t.owner), enc(&t.project));
    let p: Project = get(auth, &format!("{project}?api-version={V}"))?;
    let Some(team) = p.default_team else { return Ok(Vec::new()) };
    let members: List<Member> = get(auth, &format!("{project}/teams/{}/members?$top=100&api-version={V}", enc(&team.id)))?;
    let mut out: Vec<Person> = Vec::new();
    for m in members.value.into_iter().map(|m| m.identity).filter(|i| !i.is_container && !i.unique_name.is_empty()) {
        if !out.iter().any(|p| p.id == m.unique_name) {
            let name = if m.display_name.is_empty() { m.unique_name.clone() } else { m.display_name };
            out.push(Person { id: m.unique_name, name });
        }
    }
    Ok(out)
}

fn add(path: &str, value: Value) -> Value {
    json!({ "op": "add", "path": path, "value": value })
}

/// The JSON Patch operations of a new work item: empty fields are left out; a Bug's text goes
/// to its repro steps.
fn operations(t: &Target, kind: &str, new: &NewIssue) -> Value {
    let mut ops = vec![add("/fields/System.Title", json!(new.title))];
    if !new.description.trim().is_empty() {
        let field = if kind.eq_ignore_ascii_case("bug") { "Microsoft.VSTS.TCM.ReproSteps" } else { "System.Description" };
        ops.push(add(&format!("/fields/{field}"), json!(from_text(&new.description))));
    }
    if !new.labels.is_empty() {
        ops.push(add("/fields/System.Tags", json!(new.labels.join("; "))));
    }
    if let Some(a) = new.assignees.first() {
        ops.push(add("/fields/System.AssignedTo", json!(a)));
    }
    if let Some(id) = new.epic.as_deref().and_then(|e| e.trim().parse::<u32>().ok()) {
        let url = format!("https://dev.azure.com/{}/_apis/wit/workItems/{id}", enc(&t.owner));
        ops.push(add("/relations/-", json!({ "rel": PARENT, "url": url })));
    }
    Value::Array(ops)
}

/// Creates the work item; a token that may only read is told which scope it needs.
pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    let Some(kind) = new.kind.as_deref().map(str::trim).filter(|k| !k.is_empty()) else {
        return Err(Error::Other("Choose a work item type.".into()));
    };
    let url = format!("{}/_apis/wit/workitems/${}?api-version={V}", project_base(t), enc(kind));
    let created: WorkItem = post_json_patch(auth, &url, operations(t, kind, new)).map_err(|e| match e {
        Error::Forbidden(m) => {
            let hint = "creating work items needs a token with the Work Items (Read & write) scope";
            Error::Forbidden(if m.is_empty() { hint.to_owned() } else { format!("{m}; {hint}") })
        }
        e => e,
    })?;
    Ok(to_issue(t, created))
}

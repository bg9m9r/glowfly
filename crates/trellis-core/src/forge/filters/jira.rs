//! Issue searches (JQL) and picker options (labels, fix versions, sprints) on Jira Cloud and
//! Jira Data Center.

use super::super::jira::{api, search};
use super::super::{Auth, Issue, Provider, Target, enc, get};
use super::{Conditions, FilterKind, FilterOptions, Milestone, Named, Sprint, Who, esc_quoted};
use crate::error::Result;
use serde::Deserialize;

/// Boards whose sprints are listed.
const MAX_BOARDS: usize = 5;

/// The JQL query for open issues matching `c`, limited to `project` when one is set.
pub(crate) fn jql(c: &Conditions, project: &str) -> String {
    let quoted = |s: &str| format!("\"{}\"", esc_quoted(s.trim(), '"'));
    let mut q = String::from("statusCategory != Done");
    if !project.trim().is_empty() {
        q.push_str(&format!(" AND project = {}", quoted(project)));
    }
    match c.assignee {
        Who::Me => q.push_str(" AND assignee = currentUser()"),
        Who::Nobody => q.push_str(" AND assignee is EMPTY"),
        Who::Anyone => {}
    }
    if c.author == Who::Me {
        q.push_str(" AND reporter = currentUser()");
    }
    for l in &c.labels {
        q.push_str(&format!(" AND labels = {}", quoted(l)));
    }
    match &c.milestone {
        Milestone::Named { title, .. } => q.push_str(&format!(" AND fixVersion = {}", quoted(title))),
        Milestone::None => q.push_str(" AND fixVersion is EMPTY"),
        Milestone::Any => {}
    }
    match &c.sprint {
        Sprint::Open => q.push_str(" AND sprint in openSprints()"),
        Sprint::Named { id, .. } => q.push_str(&format!(" AND sprint = {id}")),
        Sprint::Any => {}
    }
    q.push_str(" ORDER BY updated DESC");
    q
}

pub(crate) fn find_issues(t: &Target, auth: &Auth, c: &Conditions) -> Result<Vec<Issue>> {
    search(t, auth, &jql(c, &t.project))
}

#[derive(Deserialize)]
struct Values<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
}

#[derive(Deserialize)]
struct Version {
    name: String,
    #[serde(default)]
    released: bool,
}

#[derive(Deserialize)]
struct Board {
    id: u64,
}

#[derive(Deserialize)]
struct ApiSprint {
    id: u64,
    #[serde(default)]
    name: String,
}

/// Labels (Jira Cloud), and with a project set its unreleased fix versions and its scrum boards'
/// active and future sprints. Sprints are left out where Jira has no boards to list.
pub(crate) fn options(t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
    let mut out = FilterOptions::default();
    if kind == FilterKind::Prs {
        return Ok(out);
    }
    let base = api(&t.host, t.provider);
    if t.provider == Provider::JiraCloud {
        out.labels = get::<Values<String>>(auth, &format!("{base}/label?maxResults=1000"))?.values;
    }
    let project = t.project.trim();
    if project.is_empty() {
        return Ok(out);
    }
    let versions: Vec<Version> = get(auth, &format!("{base}/project/{}/versions", enc(project)))?;
    out.milestones = versions.into_iter().filter(|v| !v.released).map(|v| Named { key: v.name.clone(), title: v.name }).collect();
    let agile = format!("https://{}/rest/agile/1.0", t.host);
    let boards = get::<Values<Board>>(auth, &format!("{agile}/board?projectKeyOrId={}&type=scrum", enc(project)));
    for b in boards.map(|b| b.values).unwrap_or_default().into_iter().take(MAX_BOARDS) {
        let Ok(sprints) = get::<Values<ApiSprint>>(auth, &format!("{agile}/board/{}/sprint?state=active,future", b.id)) else { continue };
        for s in sprints.values {
            let key = s.id.to_string();
            if !out.sprints.iter().any(|n| n.key == key) {
                out.sprints.push(Named { key, title: s.name });
            }
        }
    }
    Ok(out)
}

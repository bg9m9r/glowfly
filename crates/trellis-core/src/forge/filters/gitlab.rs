//! Issue searches, the signed-in user and picker options on GitLab.com and GitLab Self-Managed.

use super::super::gitlab::{ApiIssue, User, project_api, to_issue};
use super::super::{Auth, Issue, Target, enc, get, page};
use super::{Conditions, FilterKind, FilterOptions, Me, Milestone, Named, Who, matching};
use crate::error::Result;
use serde::Deserialize;

/// The query of an issue list call for `c`; `username` is the signed-in user's.
pub(crate) fn issue_params(c: &Conditions, username: &str) -> String {
    let mut q = String::from("state=opened&per_page=100&order_by=updated_at&scope=all");
    match c.assignee {
        Who::Me => q.push_str(&format!("&assignee_username={}", enc(username))),
        Who::Nobody => q.push_str("&assignee_id=None"),
        Who::Anyone => {}
    }
    if c.author == Who::Me {
        q.push_str(&format!("&author_username={}", enc(username)));
    }
    if !c.labels.is_empty() {
        let labels: Vec<String> = c.labels.iter().map(|l| enc(l.trim())).collect();
        q.push_str(&format!("&labels={}", labels.join(",")));
    }
    match &c.milestone {
        Milestone::Named { title, .. } => q.push_str(&format!("&milestone={}", enc(title))),
        Milestone::None => q.push_str("&milestone=None"),
        Milestone::Any => {}
    }
    q
}

pub(crate) fn me(t: &Target, auth: &Auth) -> Result<Me> {
    let u: User = get(auth, &format!("https://{}/api/v4/user", t.host))?;
    let name = if u.name.is_empty() { u.username.clone() } else { u.name };
    Ok(Me { id: u.username, name })
}

pub(crate) fn find_issues(t: &Target, auth: &Auth, c: &Conditions, known: Option<&Me>) -> Result<Vec<Issue>> {
    let username = match known {
        _ if !matching::needs_me(FilterKind::Issues, c) => String::new(),
        Some(m) => m.id.clone(),
        None => me(t, auth)?.id,
    };
    let url = format!("{}/issues?{}", project_api(t), issue_params(c, &username));
    Ok(page::by_link::<ApiIssue>(auth, url)?.into_iter().map(to_issue).collect())
}

/// The project's labels and active milestones (a milestone's title is its key).
pub(crate) fn options(t: &Target, auth: &Auth) -> Result<FilterOptions> {
    #[derive(Deserialize)]
    struct ApiMilestone {
        title: String,
    }
    let labels = labels(t, auth)?;
    let milestones: Vec<ApiMilestone> = page::by_link(auth, format!("{}/milestones?state=active&per_page=100", project_api(t)))?;
    Ok(FilterOptions {
        labels,
        milestones: milestones.into_iter().map(|m| Named { key: m.title.clone(), title: m.title }).collect(),
        sprints: Vec::new(),
    })
}

/// The project's label names (the filter dialog and the Create issue dialog).
pub(crate) fn labels(t: &Target, auth: &Auth) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Label {
        name: String,
    }
    let labels: Vec<Label> = page::by_link(auth, format!("{}/labels?per_page=100", project_api(t)))?;
    Ok(labels.into_iter().map(|l| l.name).collect())
}

//! Issue searches, the signed-in user and picker options on GitHub and GitHub Enterprise Server.

use super::super::github::{self as gh, ApiIssue, Label, repo_api, to_issue};
use super::super::{Auth, Issue, Target, enc, page};
use super::{Conditions, FilterKind, FilterOptions, Me, Milestone, Named, Who, matching};
use crate::error::Result;
use serde::Deserialize;

/// The query of an issue list call for `c`; `login` is the signed-in user's.
pub(crate) fn issue_params(c: &Conditions, login: &str) -> String {
    let mut q = String::from("state=open&per_page=100&sort=updated");
    match c.assignee {
        Who::Me => q.push_str(&format!("&assignee={}", enc(login))),
        Who::Nobody => q.push_str("&assignee=none"),
        Who::Anyone => {}
    }
    if c.author == Who::Me {
        q.push_str(&format!("&creator={}", enc(login)));
    }
    if c.mentions_me {
        q.push_str(&format!("&mentioned={}", enc(login)));
    }
    if !c.labels.is_empty() {
        let labels: Vec<String> = c.labels.iter().map(|l| enc(l.trim())).collect();
        q.push_str(&format!("&labels={}", labels.join(",")));
    }
    match &c.milestone {
        Milestone::Named { key, .. } => q.push_str(&format!("&milestone={}", enc(key))),
        Milestone::None => q.push_str("&milestone=none"),
        Milestone::Any => {}
    }
    q
}

pub(crate) fn me(t: &Target, auth: &Auth) -> Result<Me> {
    let u = gh::me(&t.host, t.provider, auth)?;
    let name = u.name.filter(|n| !n.is_empty()).unwrap_or_else(|| u.login.clone());
    Ok(Me { id: u.login, name })
}

pub(crate) fn find_issues(t: &Target, auth: &Auth, c: &Conditions, known: Option<&Me>) -> Result<Vec<Issue>> {
    let login = match known {
        _ if !matching::needs_me(FilterKind::Issues, c) => String::new(),
        Some(m) => m.id.clone(),
        None => me(t, auth)?.id,
    };
    let items: Vec<ApiIssue> = page::by_link(auth, format!("{}/issues?{}", repo_api(t), issue_params(c, &login)))?;
    Ok(items.into_iter().filter(|i| i.pull_request.is_none()).map(to_issue).collect())
}

/// Labels and open milestones; GitHub shares them between issues and pull requests.
pub(crate) fn options(t: &Target, auth: &Auth) -> Result<FilterOptions> {
    #[derive(Deserialize)]
    struct ApiMilestone {
        number: u64,
        title: String,
    }
    let labels: Vec<Label> = page::by_link(auth, format!("{}/labels?per_page=100", repo_api(t)))?;
    let milestones: Vec<ApiMilestone> = page::by_link(auth, format!("{}/milestones?state=open&per_page=100", repo_api(t)))?;
    Ok(FilterOptions {
        labels: labels.into_iter().map(|l| l.name).collect(),
        milestones: milestones.into_iter().map(|m| Named { key: m.number.to_string(), title: m.title }).collect(),
        sprints: Vec::new(),
    })
}

//! Issue searches, the signed-in user, their teams and picker options on GitHub and GitHub
//! Enterprise Server.

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
    Ok(Me { id: u.login, name, ..Me::default() })
}

/// A team's identity in team review requests and `Me::teams`: "org/slug".
pub(crate) fn team_key(org: &str, slug: &str) -> String {
    format!("{org}/{slug}")
}

/// A team named in a pull request's `requested_teams`, or a team's parent.
#[derive(Deserialize)]
pub(crate) struct TeamRef {
    pub(crate) slug: String,
}

/// The teams the signed-in user belongs to, and each one's direct parent (a request to a parent
/// team reaches its child teams' members), as "org/slug". Needs a token that can read org
/// membership; otherwise the call fails and the caller treats that as no teams.
pub(crate) fn my_teams(t: &Target, auth: &Auth) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct ApiTeam {
        slug: String,
        organization: gh::Login,
        #[serde(default)]
        parent: Option<TeamRef>,
    }
    let teams: Vec<ApiTeam> = page::by_link(auth, format!("{}/user/teams?per_page=100", gh::api(&t.host, t.provider)))?;
    let mut out: Vec<String> = Vec::new();
    for team in teams {
        let org = &team.organization.login;
        for key in std::iter::once(team_key(org, &team.slug)).chain(team.parent.map(|p| team_key(org, &p.slug))) {
            if !out.iter().any(|k| k.eq_ignore_ascii_case(&key)) {
                out.push(key);
            }
        }
    }
    Ok(out)
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
    let labels = labels(t, auth)?;
    let milestones: Vec<ApiMilestone> = page::by_link(auth, format!("{}/milestones?state=open&per_page=100", repo_api(t)))?;
    Ok(FilterOptions {
        labels,
        milestones: milestones.into_iter().map(|m| Named { key: m.number.to_string(), title: m.title }).collect(),
        sprints: Vec::new(),
    })
}

/// The repository's label names (the filter dialog and the Create issue dialog).
pub(crate) fn labels(t: &Target, auth: &Auth) -> Result<Vec<String>> {
    let labels: Vec<Label> = page::by_link(auth, format!("{}/labels?per_page=100", repo_api(t)))?;
    Ok(labels.into_iter().map(|l| l.name).collect())
}

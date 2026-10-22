//! Issue searches, the signed-in user and picker options on Bitbucket Cloud, and the signed-in
//! user on Bitbucket Data Center (which has no issue tracker).

use super::super::bitbucket::{CLOUD, CloudIssue, CloudUser, Name, cloud_list, cloud_repo, to_issue};
use super::super::bitbucket_dc::BitbucketDc;
use super::super::{Auth, Forge, Issue, Target, enc, get};
use super::{Conditions, FilterKind, FilterOptions, Me, Milestone, Named, Who, esc_quoted, matching};
use crate::error::Result;

/// The `q=` query of an issue list call for `c`; `uuid` is the signed-in user's account id.
pub(crate) fn cloud_q(c: &Conditions, uuid: &str) -> String {
    let quoted = |s: &str| format!("\"{}\"", esc_quoted(s, '"'));
    let mut q = String::from(r#"(state="new" OR state="open")"#);
    match c.assignee {
        Who::Me => q.push_str(&format!(" AND assignee.uuid={}", quoted(uuid))),
        Who::Nobody => q.push_str(" AND assignee = null"),
        Who::Anyone => {}
    }
    if c.author == Who::Me {
        q.push_str(&format!(" AND reporter.uuid={}", quoted(uuid)));
    }
    match &c.milestone {
        Milestone::Named { title, .. } => q.push_str(&format!(" AND milestone.name={}", quoted(title))),
        Milestone::None => q.push_str(" AND milestone = null"),
        Milestone::Any => {}
    }
    q
}

pub(crate) fn me(auth: &Auth) -> Result<Me> {
    let u: CloudUser = get(auth, &format!("{CLOUD}/user"))?;
    Ok(Me { id: u.uuid, name: u.display_name, ..Me::default() })
}

/// Data Center has no "current user" resource; whoami gives the username pull requests carry.
pub(crate) fn dc_me(t: &Target, auth: &Auth) -> Result<Me> {
    let name = BitbucketDc.verify(&t.account(), auth)?;
    Ok(Me { id: name.clone(), name, ..Me::default() })
}

/// Repositories with the issue tracker turned off answer 404: no issues.
pub(crate) fn find_issues(t: &Target, auth: &Auth, c: &Conditions, known: Option<&Me>) -> Result<Vec<Issue>> {
    let uuid = match known {
        _ if !matching::needs_me(FilterKind::Issues, c) => String::new(),
        Some(m) => m.id.clone(),
        None => me(auth)?.id,
    };
    let url = format!("{}/issues?q={}&sort=-updated_on&pagelen=50", cloud_repo(t), enc(&cloud_q(c, &uuid)));
    match cloud_list::<CloudIssue>(auth, url) {
        Ok(items) => Ok(items.into_iter().map(to_issue).collect()),
        Err(e) if e.is_not_found() => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// The issue tracker's milestones; pull requests have none to pick.
pub(crate) fn options(t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
    if kind == FilterKind::Prs {
        return Ok(FilterOptions::default());
    }
    let milestones = match cloud_list::<Name>(auth, format!("{}/milestones?pagelen=100", cloud_repo(t))) {
        Ok(list) => list,
        Err(e) if e.is_not_found() => Vec::new(),
        Err(e) => return Err(e),
    };
    let milestones = milestones.into_iter().map(|m| Named { key: m.name.clone(), title: m.name }).collect();
    Ok(FilterOptions { milestones, ..FilterOptions::default() })
}

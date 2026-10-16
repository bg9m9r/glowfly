//! Work item searches (WIQL), the signed-in user and tags on Azure DevOps.

use super::super::azure::{List, Name, project_base, wiql};
use super::super::{Auth, Issue, Target, enc, get};
use super::{Conditions, FilterKind, FilterOptions, Me, Who};
use crate::error::Result;
use serde::Deserialize;

/// The WIQL `WHERE` clause for open work items in the project matching `c`. `@Me` and
/// `@RecentMentions` (the last 30 days) are resolved by the server.
pub(crate) fn wiql_where(c: &Conditions) -> String {
    let mut w = String::from("[System.TeamProject] = @project AND [System.State] NOT IN ('Closed', 'Done', 'Removed')");
    match c.assignee {
        Who::Me => w.push_str(" AND [System.AssignedTo] = @Me"),
        Who::Nobody => w.push_str(" AND [System.AssignedTo] = ''"),
        Who::Anyone => {}
    }
    if c.author == Who::Me {
        w.push_str(" AND [System.CreatedBy] = @Me");
    }
    if c.mentions_me {
        w.push_str(" AND [System.Id] IN (@RecentMentions)");
    }
    for tag in &c.labels {
        w.push_str(&format!(" AND [System.Tags] CONTAINS '{}'", tag.trim().replace('\'', "''")));
    }
    w
}

/// A work item's `System.Tags` ("a; b") as a list.
pub(crate) fn tags(field: Option<&str>) -> Vec<String> {
    field.into_iter().flat_map(|t| t.split(';')).map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

pub(crate) fn find_issues(t: &Target, auth: &Auth, c: &Conditions) -> Result<Vec<Issue>> {
    wiql(t, auth, &wiql_where(c))
}

pub(crate) fn me(t: &Target, auth: &Auth) -> Result<Me> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ConnectionData {
        authenticated_user: User,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct User {
        #[serde(default)]
        id: String,
        #[serde(default)]
        provider_display_name: String,
    }
    let url = format!("https://dev.azure.com/{}/_apis/connectionData?api-version=7.1-preview", enc(&t.owner));
    let user = get::<ConnectionData>(auth, &url)?.authenticated_user;
    Ok(Me { id: user.id, name: user.provider_display_name })
}

/// The project's work item tags; pull request labels are only those seen on loaded ones.
pub(crate) fn options(t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
    if kind == FilterKind::Prs {
        return Ok(FilterOptions::default());
    }
    let tags: List<Name> = get(auth, &format!("{}/_apis/wit/tags?api-version=7.1-preview.1", project_base(t)))?;
    Ok(FilterOptions { labels: tags.value.into_iter().map(|n| n.name).collect(), ..FilterOptions::default() })
}

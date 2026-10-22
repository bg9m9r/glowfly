//! Work item searches (WIQL), the signed-in user, their groups and tags on Azure DevOps.

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
    Ok(Me { id: user.id, name: user.provider_display_name, ..Me::default() })
}

/// Descriptors asked about per identities call (keeps URLs around 3 KB).
const GROUPS_PER_CALL: usize = 25;
/// Group and team memberships read at most.
const MAX_GROUPS: usize = 200;

fn identities_url(t: &Target) -> String {
    format!("https://vssps.dev.azure.com/{}/_apis/identities", enc(&t.owner))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ident {
    #[serde(default)]
    id: String,
    #[serde(default)]
    member_of: Vec<serde_json::Value>,
}

/// A membership descriptor: usually "type;identifier" as a string, sometimes split in an object.
fn descriptor(v: serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) if !s.is_empty() => Some(s),
        serde_json::Value::Object(o) => match (o.get("identityType")?.as_str()?, o.get("identifier")?.as_str()?) {
            (kind, id) if !kind.is_empty() && !id.is_empty() => Some(format!("{kind};{id}")),
            _ => None,
        },
        _ => None,
    }
}

/// The identity ids of the groups and teams the identity `me_id` belongs to, directly or
/// through other groups: the ids container reviewers carry. Needs the Identity (Read) scope;
/// the first failed call is returned and the caller treats it as no groups.
pub(crate) fn my_groups(t: &Target, auth: &Auth, me_id: &str) -> Result<Vec<String>> {
    if me_id.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{}?identityIds={}&queryMembership=expanded&api-version=7.1", identities_url(t), enc(me_id));
    let mut descriptors: Vec<String> = Vec::new();
    for d in get::<List<Option<Ident>>>(auth, &url)?.value.into_iter().flatten().flat_map(|i| i.member_of).filter_map(descriptor) {
        if !descriptors.contains(&d) {
            descriptors.push(d);
        }
    }
    descriptors.truncate(MAX_GROUPS);
    let mut ids: Vec<String> = Vec::new();
    for chunk in descriptors.chunks(GROUPS_PER_CALL) {
        let list: Vec<String> = chunk.iter().map(|d| enc(d)).collect();
        let url = format!("{}?descriptors={}&queryMembership=none&api-version=7.1", identities_url(t), list.join(","));
        for id in get::<List<Option<Ident>>>(auth, &url)?.value.into_iter().flatten().map(|i| i.id) {
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

/// The project's work item tags; pull request labels are only those seen on loaded ones.
pub(crate) fn options(t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
    if kind == FilterKind::Prs {
        return Ok(FilterOptions::default());
    }
    Ok(FilterOptions { labels: tag_names(t, auth)?, ..FilterOptions::default() })
}

/// The project's work item tags.
pub(crate) fn tag_names(t: &Target, auth: &Auth) -> Result<Vec<String>> {
    let tags: List<Name> = get(auth, &format!("{}/_apis/wit/tags?api-version=7.1-preview.1", project_base(t)))?;
    Ok(tags.value.into_iter().map(|n| n.name).collect())
}

//! Azure DevOps work items as issues: the listed fields, a batch lookup by id, and WIQL
//! queries, for the ISSUES list, filters, linked work items and the work item panel.

use super::azure::{Identity, List, V, project_base, work_item_url};
use super::page::MAX_ITEMS;
use super::{Auth, Issue, IssueFacets, Target, get, post};
use crate::error::Result;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Default)]
pub(super) struct Fields {
    #[serde(rename = "System.Title", default)]
    pub(super) title: String,
    #[serde(rename = "System.State", default)]
    pub(super) state: String,
    #[serde(rename = "System.WorkItemType", default)]
    pub(super) kind: String,
    #[serde(rename = "System.AssignedTo", default)]
    pub(super) assigned_to: Option<Identity>,
    /// "a; b"
    #[serde(rename = "System.Tags", default)]
    pub(super) tags: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct WorkItem {
    pub(super) id: u32,
    pub(super) fields: Fields,
}

pub(super) fn to_issue(t: &Target, w: WorkItem) -> Issue {
    let key = w.id.to_string();
    Issue {
        url: work_item_url(t, &key),
        key,
        title: w.fields.title,
        state: w.fields.state,
        kind: w.fields.kind,
        assignee: w.fields.assigned_to.map(|a| a.display_name),
        facets: IssueFacets { labels: super::filters::azure::tags(w.fields.tags.as_deref()), milestone: None },
    }
}

/// Open work items in this project matching the WIQL `where` clause, most recently changed first.
pub(super) fn wiql(t: &Target, auth: &Auth, where_clause: &str) -> Result<Vec<Issue>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Wiql {
        #[serde(default)]
        work_items: Vec<Ref>,
    }
    #[derive(Deserialize)]
    struct Ref {
        id: u32,
    }
    let query = format!("SELECT [System.Id] FROM WorkItems WHERE {where_clause} ORDER BY [System.ChangedDate] DESC");
    let result: Wiql =
        post(auth, &format!("{}/_apis/wit/wiql?$top={MAX_ITEMS}&api-version={V}", project_base(t)), json!({ "query": query }))?;
    let ids: Vec<String> = result.work_items.iter().map(|w| w.id.to_string()).collect();
    let mut items = work_items(t, auth, &ids)?;
    items.sort_by_key(|w| ids.iter().position(|i| *i == w.key));
    Ok(items)
}

/// The work items with these ids; ids that aren't numbers, and work items that don't exist,
/// are left out.
pub(super) fn work_items(t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
    let ids: Vec<&String> = keys.iter().filter(|k| k.parse::<u32>().is_ok()).collect();
    let mut out = Vec::new();
    // The batch endpoint accepts at most 200 ids per call.
    for chunk in ids.chunks(200) {
        let list: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let url = format!(
            "{}/_apis/wit/workitems?ids={}&fields=System.Title,System.State,System.WorkItemType,System.AssignedTo,System.Tags&errorPolicy=omit&api-version={V}",
            project_base(t),
            list.join(",")
        );
        // With errorPolicy=omit, ids that do not exist come back as null entries.
        out.extend(get::<List<Option<WorkItem>>>(auth, &url)?.value.into_iter().flatten().map(|w| to_issue(t, w)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::azure::parse_remote;

    #[test]
    fn converts_work_items_and_tags() {
        let t = parse_remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app").unwrap();
        let items: List<Option<WorkItem>> = serde_json::from_str(
            r#"{"value":[{"id":5,"fields":{"System.Title":"Login page","System.State":"Active","System.WorkItemType":"User Story","System.AssignedTo":{"displayName":"Ada"}}},null]}"#,
        )
        .unwrap();
        let issue = to_issue(&t, items.value.into_iter().flatten().next().unwrap());
        assert_eq!((issue.key.as_str(), issue.kind.as_str(), issue.assignee.as_deref()), ("5", "User Story", Some("Ada")));
        assert_eq!(issue.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_workitems/edit/5");
        let w: WorkItem = serde_json::from_str(r#"{"id":5,"fields":{"System.Title":"L","System.Tags":"ux; needs design;"}}"#).unwrap();
        assert_eq!(to_issue(&t, w).facets.labels, ["ux", "needs design"]);
    }
}

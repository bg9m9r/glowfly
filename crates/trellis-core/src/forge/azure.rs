//! Azure DevOps (Azure Repos + Azure Boards), REST api-version 7.1.

pub use super::azure_remote::{credential_host, parse_host_domain, parse_remote};
use super::checks::per_pr;
use super::details::{PrDetails, checks_of};
use super::http::get_paged;
use super::page::{self, MAX_ITEMS, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, Checks, Issue, NewPullRequest, PullRequest, RefStyle, Reviewer, Target, TipQuery, azure_checks, enc, get, post,
};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub(super) static AZURE_DEVOPS: ProviderInfo = ProviderInfo {
    name: "Azure DevOps",
    host_hint: "e.g., dev.azure.com/mycompany",
    token_help: "Needs the Code (Read & write) and Work Items (Read) scopes, and Build (Read) for build status on branches.",
    env_var: Some("AZURE_DEVOPS_EXT_PAT"),
    pr_prefix: "!",
    ref_style: RefStyle::AzureBoards,
    links_issues_by_api: true,
    details_per_pr: true,
    service: &AzureDevOps,
    ..DEFAULTS
};

pub(super) const V: &str = "7.1";
/// Pull requests asked for per page.
const TOP: usize = 100;

pub(super) fn project_base(t: &Target) -> String {
    format!("https://dev.azure.com/{}/{}", enc(&t.owner), enc(&t.project))
}

pub(super) fn repo_api(t: &Target) -> String {
    format!("{}/_apis/git/repositories/{}", project_base(t), enc(&t.repo))
}

pub fn pr_url(t: &Target, id: u64) -> String {
    format!("{}/_git/{}/pullrequest/{id}", project_base(t), enc(&t.repo))
}

pub fn work_item_url(t: &Target, key: &str) -> String {
    format!("{}/_workitems/edit/{key}", project_base(t))
}

#[derive(Deserialize)]
struct List<T> {
    value: Vec<T>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Identity {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiReviewer {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    vote: i32,
    #[serde(default)]
    is_required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitRef {
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiPr {
    pull_request_id: u64,
    title: String,
    #[serde(default)]
    description: Option<String>,
    source_ref_name: String,
    target_ref_name: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    created_by: Identity,
    #[serde(default)]
    reviewers: Vec<ApiReviewer>,
    #[serde(default)]
    merge_status: Option<String>,
    #[serde(default)]
    last_merge_source_commit: Option<CommitRef>,
    #[serde(default)]
    repository: Option<ApiRepo>,
}

#[derive(Deserialize)]
struct ApiRepo {
    project: ApiProject,
}

#[derive(Deserialize)]
struct ApiProject {
    #[serde(default)]
    id: String,
}

/// The repository resource: its default branch and the project it belongs to.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoInfo {
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    project: Option<ApiProject>,
}

fn vote_label(vote: i32) -> &'static str {
    match vote {
        10 => "approved",
        5 => "approved with suggestions",
        -5 => "waiting for author",
        -10 => "rejected",
        _ => "no vote",
    }
}

fn convert(t: &Target, p: ApiPr, checks: Option<Checks>) -> PullRequest {
    PullRequest {
        id: p.pull_request_id,
        url: pr_url(t, p.pull_request_id),
        title: p.title,
        description: p.description.unwrap_or_default(),
        source: p.source_ref_name.trim_start_matches("refs/heads/").to_owned(),
        target: p.target_ref_name.trim_start_matches("refs/heads/").to_owned(),
        author: p.created_by.display_name,
        draft: p.is_draft,
        reviewers: p
            .reviewers
            .into_iter()
            .map(|r| Reviewer { name: r.display_name, state: vote_label(r.vote).into(), required: r.is_required })
            .collect(),
        head: p.last_merge_source_commit.map(|c| c.commit_id),
        merge_status: p.merge_status,
        checks,
    }
}

#[derive(Deserialize)]
struct Fields {
    #[serde(rename = "System.Title", default)]
    title: String,
    #[serde(rename = "System.State", default)]
    state: String,
    #[serde(rename = "System.WorkItemType", default)]
    kind: String,
    #[serde(rename = "System.AssignedTo", default)]
    assigned_to: Option<Identity>,
}

#[derive(Deserialize)]
struct WorkItem {
    id: u32,
    fields: Fields,
}

fn to_issue(t: &Target, w: WorkItem) -> Issue {
    let key = w.id.to_string();
    Issue {
        url: work_item_url(t, &key),
        key,
        title: w.fields.title,
        state: w.fields.state,
        kind: w.fields.kind,
        assignee: w.fields.assigned_to.map(|a| a.display_name),
    }
}

/// The AzureDevOps [`Forge`](super::Forge).
pub struct AzureDevOps;

impl super::Forge for AzureDevOps {
    fn issue_url(&self, t: &Target, key: &str) -> String {
        work_item_url(t, &enc(key))
    }

    fn token_url(&self, account: &Account) -> Option<String> {
        (!account.org.is_empty()).then(|| format!("https://dev.azure.com/{}/_usersSettings/tokens", enc(&account.org)))
    }

    fn display(&self, t: &Target) -> String {
        format!("{}/{}/{}", t.owner, t.project, t.repo)
    }

    fn repo_url(&self, t: &Target) -> String {
        let base =
            if t.host == "dev.azure.com" { format!("https://dev.azure.com/{}", enc(&t.owner)) } else { format!("https://{}", t.host) };
        format!("{base}/{}/_git/{}", enc(&t.project), enc(&t.repo))
    }

    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ConnectionData {
            authenticated_user: User,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct User {
            #[serde(default)]
            provider_display_name: String,
            #[serde(default)]
            descriptor: String,
        }
        let url = format!("https://dev.azure.com/{}/_apis/connectionData?api-version=7.1-preview", enc(&account.org));
        let user = get::<ConnectionData>(auth, &url)?.authenticated_user;
        // Unauthenticated callers get an anonymous/public identity rather than an error.
        if user.provider_display_name.is_empty()
            || user.descriptor.starts_with("System:PublicAccess")
            || user.provider_display_name.eq_ignore_ascii_case("anonymous")
        {
            return Err(Error::Auth(format!("the credentials were not accepted by {}", account.org)));
        }
        Ok(user.provider_display_name)
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let first = format!("{}/pullrequests?searchCriteria.status=active&$top={TOP}&api-version={V}", repo_api(t));
        let mut read = 0;
        let prs = page::collect(first.clone(), |url| {
            let p = get_paged::<List<ApiPr>>(auth, url)?;
            read += p.body.value.len();
            // A continuation token when the server sends one; otherwise skip what was read.
            let next = match p.continuation {
                Some(token) => Some(page::with_param(&first, "continuationToken", &token)),
                None => (p.body.value.len() >= TOP).then(|| page::with_param(&first, "$skip", &read.to_string())),
            };
            Ok(Page { items: p.body.value, next })
        })?;
        let checks = per_pr(&prs, |p| match p.repository.as_ref().filter(|r| !r.project.id.is_empty()) {
            Some(r) => azure_checks::pr_checks(t, auth, &r.project.id, p.pull_request_id),
            None => Ok(None),
        });
        Ok(prs.into_iter().zip(checks).map(|(p, c)| convert(t, p, c)).collect())
    }

    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
        let info: RepoInfo = get(auth, &format!("{}?api-version={V}", repo_api(t)))?;
        Ok(info.default_branch.map(|b| b.trim_start_matches("refs/heads/").to_owned()))
    }

    /// Build policies and statuses of each pull request, under the repository's project id.
    fn pr_details(&self, t: &Target, auth: &Auth, prs: &[PullRequest]) -> Result<Vec<PrDetails>> {
        let info: RepoInfo = get(auth, &format!("{}?api-version={V}", repo_api(t)))?;
        let project = info.project.map(|p| p.id).unwrap_or_default();
        if project.is_empty() {
            return Ok(prs.iter().map(PrDetails::empty).collect());
        }
        checks_of(prs, |p| azure_checks::pr_checks(t, auth, &project, p.id))
    }

    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        let work_items: Vec<_> = pr.issues.iter().filter(|k| k.parse::<u32>().is_ok()).map(|id| json!({ "id": id })).collect();
        let body = json!({
            "sourceRefName": format!("refs/heads/{}", pr.source),
            "targetRefName": format!("refs/heads/{}", pr.target),
            "title": pr.title,
            "description": pr.description,
            "isDraft": pr.draft,
            "workItemRefs": work_items,
        });
        let created: ApiPr = post(auth, &format!("{}/pullrequests?api-version={V}", repo_api(t)), body)?;
        Ok(convert(t, created, None))
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
        azure_checks::commit_checks(t, auth, tips)
    }

    /// Open work items assigned to the signed-in user in this project, most recently changed first.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
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
        let query = "SELECT [System.Id] FROM WorkItems \
            WHERE [System.TeamProject] = @project AND [System.AssignedTo] = @Me \
            AND [System.State] NOT IN ('Closed', 'Done', 'Removed') \
            ORDER BY [System.ChangedDate] DESC";
        let result: Wiql =
            post(auth, &format!("{}/_apis/wit/wiql?$top={MAX_ITEMS}&api-version={V}", project_base(t)), json!({ "query": query }))?;
        let ids: Vec<String> = result.work_items.iter().map(|w| w.id.to_string()).collect();
        let mut items = self.issues(t, auth, &ids)?;
        items.sort_by_key(|w| ids.iter().position(|i| *i == w.key));
        Ok(items)
    }

    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        let ids: Vec<&String> = keys.iter().filter(|k| k.parse::<u32>().is_ok()).collect();
        let mut out = Vec::new();
        // The batch endpoint accepts at most 200 ids per call.
        for chunk in ids.chunks(200) {
            let list: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            let url = format!(
                "{}/_apis/wit/workitems?ids={}&fields=System.Title,System.State,System.WorkItemType,System.AssignedTo&errorPolicy=omit&api-version={V}",
                project_base(t),
                list.join(",")
            );
            // With errorPolicy=omit, ids that do not exist come back as null entries.
            out.extend(get::<List<Option<WorkItem>>>(auth, &url)?.value.into_iter().flatten().map(|w| to_issue(t, w)));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_payloads() {
        let t = parse_remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app").unwrap();
        assert_eq!(pr_url(&t, 7), "https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app/pullrequest/7");
        assert_eq!(work_item_url(&t, "42"), "https://dev.azure.com/contoso/Fabrikam%20Web/_workitems/edit/42");
        let prs: List<ApiPr> = serde_json::from_str(
            r#"{"value":[{"pullRequestId":17,"title":"Add login","description":"AB#5","sourceRefName":"refs/heads/feature/5-login",
            "targetRefName":"refs/heads/main","isDraft":true,"createdBy":{"displayName":"Ada"},
            "reviewers":[{"displayName":"Grace","vote":10,"isRequired":true}],"mergeStatus":"succeeded",
            "lastMergeSourceCommit":{"commitId":"abc1234"}}],"count":1}"#,
        )
        .unwrap();
        let pr = convert(&t, prs.value.into_iter().next().unwrap(), None);
        assert_eq!((pr.id, pr.source.as_str(), pr.target.as_str(), pr.draft), (17, "feature/5-login", "main", true));
        assert_eq!(pr.approvals(), 1);
        assert!(pr.head_matches("abc1234ffff"));
        let items: List<Option<WorkItem>> = serde_json::from_str(
            r#"{"value":[{"id":5,"fields":{"System.Title":"Login page","System.State":"Active","System.WorkItemType":"User Story","System.AssignedTo":{"displayName":"Ada"}}},null]}"#,
        )
        .unwrap();
        let issue = to_issue(&t, items.value.into_iter().flatten().next().unwrap());
        assert_eq!((issue.key.as_str(), issue.kind.as_str(), issue.assignee.as_deref()), ("5", "User Story", Some("Ada")));
    }
}

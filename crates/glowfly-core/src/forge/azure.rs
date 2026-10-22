//! Azure DevOps (Azure Repos + Azure Boards), REST api-version 7.1.

pub use super::azure_remote::{credential_host, parse_host_domain, parse_remote};
pub(super) use super::azure_work_items::wiql;
use super::azure_work_items::work_items;
use super::checks::per_pr;
use super::details::{PrDetails, checks_of};
use super::http::get_paged;
use super::issue_details::{CreateOptions, IssueDetails, NewIssue};
use super::page::{self, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, Checks, Conditions, Entry, FilterKind, FilterOptions, Issue, Me, NewPullRequest, PrFacets, PullRequest, RefStyle,
    Reviewer, Target, TipQuery, azure_checks, azure_issues, enc, get, post,
};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub(super) static AZURE_DEVOPS: ProviderInfo = ProviderInfo {
    name: "Azure DevOps",
    host_hint: "e.g., dev.azure.com/mycompany",
    token_help: "Needs the Code (Read & write) and Work Items (Read & write, to create work items) scopes, Project and Team (Read) for assignees, Identity (Read) to count review requests to your groups, and Build (Read) for build status on branches.",
    env_var: Some("AZURE_DEVOPS_EXT_PAT"),
    pr_prefix: "!",
    ref_style: RefStyle::AzureBoards,
    links_issues_by_api: true,
    details_per_pr: true,
    issues_in_app: true,
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
pub(super) struct List<T> {
    pub(super) value: Vec<T>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct Identity {
    #[serde(default)]
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) id: String,
}

#[derive(Deserialize)]
pub(super) struct Name {
    pub(super) name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiReviewer {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    id: String,
    /// A group or team rather than a person.
    #[serde(default)]
    is_container: bool,
    #[serde(default)]
    vote: i32,
    #[serde(default)]
    is_required: bool,
    #[serde(default)]
    has_declined: bool,
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
    #[serde(default)]
    labels: Vec<Name>,
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
pub(super) struct RepoInfo {
    #[serde(default)]
    pub(super) id: String,
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
    let people = || p.reviewers.iter().filter(|r| !r.is_container);
    let facets = PrFacets {
        author_id: p.created_by.id.clone(),
        reviewer_ids: people().map(|r| r.id.clone()).collect(),
        awaiting_ids: people().filter(|r| r.vote == 0 && !r.has_declined).map(|r| r.id.clone()).collect(),
        // A group's vote changes when a member votes for it, so 0 means it is still pending.
        awaiting_team_ids: p
            .reviewers
            .iter()
            .filter(|r| r.is_container && r.vote == 0 && !r.has_declined && !r.id.is_empty())
            .map(|r| r.id.clone())
            .collect(),
        labels: p.labels.iter().map(|l| l.name.clone()).collect(),
        ..PrFacets::default()
    };
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
        facets,
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
        let fork = super::pr_heads::azure_fork_source(t, auth, pr)?;
        let mut body = json!({
            "sourceRefName": format!("refs/heads/{}", pr.source),
            "targetRefName": format!("refs/heads/{}", pr.target),
            "title": pr.title,
            "description": pr.description,
            "isDraft": pr.draft,
            "workItemRefs": work_items,
        });
        if let Some(f) = fork {
            body["forkSource"] = f;
        }
        let created: ApiPr = post(auth, &format!("{}/pullrequests?api-version={V}", repo_api(t)), body)?;
        Ok(convert(t, created, None))
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
        azure_checks::commit_checks(t, auth, tips)
    }

    fn check_list(&self, t: &Target, auth: &Auth, q: &super::CheckQuery) -> Result<super::CheckList> {
        super::azure_check_list::check_list(t, auth, q)
    }

    /// Open work items assigned to the signed-in user in this project, most recently changed first.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        wiql(t, auth, &super::filters::azure::wiql_where(&super::Builtin::IssuesAssignedToMe.conditions()))
    }

    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        work_items(t, auth, keys)
    }

    fn me(&self, t: &Target, auth: &Auth) -> Result<Me> {
        super::filters::azure::me(t, auth)
    }

    fn my_teams(&self, t: &Target, auth: &Auth, me: &Me) -> Result<Vec<String>> {
        super::filters::azure::my_groups(t, auth, &me.id)
    }

    fn find_issues(&self, t: &Target, auth: &Auth, c: &Conditions, _me: Option<&Me>) -> Result<Vec<Issue>> {
        super::filters::azure::find_issues(t, auth, c)
    }

    fn filter_options(&self, t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
        super::filters::azure::options(t, auth, kind)
    }

    fn issue_details(&self, t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
        azure_issues::details(t, auth, key)
    }

    fn issue_create_options(&self, t: &Target, auth: &Auth, _project: &str) -> Result<CreateOptions> {
        azure_issues::options(t, auth)
    }

    fn create_issue(&self, t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
        azure_issues::create(t, auth, new)
    }

    fn post_issue_comment(&self, t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
        azure_issues::post_comment(t, auth, key, body)
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

        // Filter facets: identity ids (not groups) and labels.
        let p: ApiPr = serde_json::from_str(
            r#"{"pullRequestId":1,"title":"T","sourceRefName":"refs/heads/x","targetRefName":"refs/heads/main","createdBy":{"id":"a-1"},
            "reviewers":[{"id":"g-1","vote":0},{"id":"l-1","vote":10},{"id":"team","vote":0,"isContainer":true},
            {"id":"grp","vote":0,"isContainer":true},{"id":"done","vote":10,"isContainer":true},
            {"id":"no","vote":0,"isContainer":true,"hasDeclined":true},{"id":"d-1","vote":0,"hasDeclined":true}],"labels":[{"name":"ux"}]}"#,
        )
        .unwrap();
        let f = convert(&t, p, None).facets;
        assert_eq!(
            (f.author_id.as_str(), f.awaiting_ids, f.reviewer_ids, f.labels),
            ("a-1", vec!["g-1".to_owned()], vec!["g-1".to_owned(), "l-1".into(), "d-1".into()], vec!["ux".to_owned()]),
            "people only; a declined reviewer isn't awaited"
        );
        assert_eq!(f.awaiting_team_ids, ["team", "grp"], "pending groups that haven't declined");
    }
}

//! Azure DevOps (Azure Repos + Azure Boards), REST api-version 7.1.

use super::{Account, Auth, Issue, NewPullRequest, Provider, PullRequest, Reviewer, Target, dec, enc, get, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;

const V: &str = "7.1";

/// Recognises dev.azure.com, *.visualstudio.com and SSH remote URLs.
pub fn parse_remote(url: &str) -> Option<Target> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let target = |host: String, org: &str, project: &str, repo: &str| Target {
        provider: Provider::AzureDevOps,
        host,
        owner: dec(org),
        project: dec(project),
        repo: dec(repo),
    };

    // SSH: git@ssh.dev.azure.com:v3/org/project/repo or org@vs-ssh.visualstudio.com:v3/...
    if url.contains("ssh.dev.azure.com") || url.contains("vs-ssh.visualstudio.com") {
        let (_, path) = url.split_once("v3/")?;
        let mut segs = path.split('/');
        let (org, project, repo) = (segs.next()?, segs.next()?, segs.next()?);
        let host = if url.contains("visualstudio.com") { format!("{}.visualstudio.com", dec(org)) } else { "dev.azure.com".into() };
        return Some(target(host, org, project, repo));
    }

    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = rest.split_once('/')?;
    let host = authority.rsplit('@').next()?.to_ascii_lowercase();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let git = segs.iter().position(|s| *s == "_git")?;
    let repo = *segs.get(git + 1)?;
    let before: Vec<&str> = segs[..git].iter().copied().filter(|s| !s.eq_ignore_ascii_case("DefaultCollection")).collect();

    if host == "dev.azure.com" {
        let org = *before.first()?;
        // A repo named after its project can omit the project segment.
        let project = before.get(1).copied().unwrap_or(repo);
        Some(target(host, org, project, repo))
    } else if let Some(org) = host.strip_suffix(".visualstudio.com") {
        let org = org.to_owned();
        let project = before.last().copied().unwrap_or(repo);
        Some(target(host, &org, project, repo))
    } else {
        None
    }
}

/// "dev.azure.com/mycompany", a full URL, or "mycompany.visualstudio.com" → (host, org).
pub fn parse_host_domain(text: &str) -> Option<(String, String)> {
    let t = text.trim();
    let t = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(t);
    let mut parts = t.split('/').filter(|s| !s.is_empty());
    let host = parts.next()?.rsplit('@').next()?.to_ascii_lowercase();
    if host == "dev.azure.com" {
        Some((host, dec(parts.next()?)))
    } else if let Some(org) = host.strip_suffix(".visualstudio.com") {
        (!org.is_empty()).then(|| (host.clone(), org.to_owned()))
    } else {
        None
    }
}

pub fn credential_host(account: &Account) -> String {
    account.host.clone()
}

fn project_base(t: &Target) -> String {
    format!("https://dev.azure.com/{}/{}", enc(&t.owner), enc(&t.project))
}

fn repo_api(t: &Target) -> String {
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

fn convert(t: &Target, p: ApiPr) -> PullRequest {
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
    }
}

pub fn pull_requests(t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
    let url = format!("{}/pullrequests?searchCriteria.status=active&$top=200&api-version={V}", repo_api(t));
    Ok(get::<List<ApiPr>>(auth, &url)?.value.into_iter().map(|p| convert(t, p)).collect())
}

pub fn default_branch(t: &Target, auth: &Auth) -> Result<Option<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RepoInfo {
        #[serde(default)]
        default_branch: Option<String>,
    }
    let info: RepoInfo = get(auth, &format!("{}?api-version={V}", repo_api(t)))?;
    Ok(info.default_branch.map(|b| b.trim_start_matches("refs/heads/").to_owned()))
}

pub fn create_pull_request(t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
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
    Ok(convert(t, created))
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

pub fn work_items(t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
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

/// Open work items assigned to the signed-in user in this project, most recently changed first.
pub fn my_work_items(t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
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
    let result: Wiql = post(auth, &format!("{}/_apis/wit/wiql?$top=100&api-version={V}", project_base(t)), json!({ "query": query }))?;
    let ids: Vec<String> = result.work_items.iter().map(|w| w.id.to_string()).collect();
    let mut items = work_items(t, auth, &ids)?;
    items.sort_by_key(|w| ids.iter().position(|i| *i == w.key));
    Ok(items)
}

pub fn verify(account: &Account, auth: &Auth) -> Result<String> {
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

/// The AzureDevOps [`Forge`](super::Forge).
pub struct AzureDevOps;

impl super::Forge for AzureDevOps {
    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        verify(account, auth)
    }
    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        pull_requests(t, auth)
    }
    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
        default_branch(t, auth)
    }
    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        create_pull_request(t, auth, pr)
    }
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        my_work_items(t, auth)
    }
    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        work_items(t, auth, keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_azure_remotes() {
        let p = |u: &str| parse_remote(u).map(|t| (t.host, t.owner, t.project, t.repo));
        let s = |a: &str, b: &str, c: &str, d: &str| Some((a.to_owned(), b.to_owned(), c.to_owned(), d.to_owned()));
        assert_eq!(
            p("https://contoso@dev.azure.com/contoso/Fabrikam%20Web/_git/web-app"),
            s("dev.azure.com", "contoso", "Fabrikam Web", "web-app")
        );
        assert_eq!(p("https://dev.azure.com/contoso/Solo/_git/Solo/"), s("dev.azure.com", "contoso", "Solo", "Solo"));
        assert_eq!(
            p("https://contoso.visualstudio.com/DefaultCollection/Fabrikam/_git/api"),
            s("contoso.visualstudio.com", "contoso", "Fabrikam", "api")
        );
        assert_eq!(p("https://contoso.visualstudio.com/_git/Solo"), s("contoso.visualstudio.com", "contoso", "Solo", "Solo"));
        assert_eq!(p("git@ssh.dev.azure.com:v3/contoso/Fabrikam/api"), s("dev.azure.com", "contoso", "Fabrikam", "api"));
        assert_eq!(
            p("contoso@vs-ssh.visualstudio.com:v3/contoso/Fabrikam/api"),
            s("contoso.visualstudio.com", "contoso", "Fabrikam", "api")
        );
        assert_eq!(p("https://github.com/owner/repo.git"), None);
        assert_eq!(parse_host_domain("https://user@dev.azure.com/my%20co/x"), Some(("dev.azure.com".into(), "my co".into())));
        assert_eq!(parse_host_domain("dev.azure.com"), None);
    }

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
        let pr = convert(&t, prs.value.into_iter().next().unwrap());
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

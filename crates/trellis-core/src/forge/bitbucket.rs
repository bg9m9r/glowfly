//! Bitbucket Cloud (API 2.0) and Bitbucket Data Center / Server (REST 1.0).

use super::page::{self, Page};
use super::{Account, Auth, Issue, NewPullRequest, Provider, PullRequest, Reviewer, Target, enc, get, get_text, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

const CLOUD: &str = "https://api.bitbucket.org/2.0";

fn cloud_repo(t: &Target) -> String {
    format!("{CLOUD}/repositories/{}/{}", enc(&t.owner), enc(&t.repo))
}

fn dc_repo(t: &Target) -> String {
    format!("https://{}/rest/api/1.0/projects/{}/repos/{}", t.host, enc(&t.owner), enc(&t.repo))
}

/// A page of a list: Cloud links the next page; Data Center gives the next start index.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Values<T> {
    values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    is_last_page: Option<bool>,
    #[serde(default)]
    next_page_start: Option<u64>,
}

fn cloud_list<T: DeserializeOwned>(auth: &Auth, first: String) -> Result<Vec<T>> {
    page::collect(first, |url| {
        let v: Values<T> = get(auth, url)?;
        Ok(Page { items: v.values, next: v.next })
    })
}

fn dc_list<T: DeserializeOwned>(auth: &Auth, first: String) -> Result<Vec<T>> {
    page::collect(first.clone(), |url| {
        let v: Values<T> = get(auth, url)?;
        let next = match (v.is_last_page, v.next_page_start) {
            (Some(false), Some(start)) => Some(page::with_param(&first, "start", &start.to_string())),
            _ => None,
        };
        Ok(Page { items: v.values, next })
    })
}

#[derive(Deserialize, Default)]
struct CloudUser {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    uuid: String,
}

pub fn verify(account: &Account, auth: &Auth) -> Result<String> {
    if account.provider == Provider::Bitbucket {
        let u: CloudUser = get(auth, &format!("{CLOUD}/user"))?;
        return Ok(u.display_name);
    }
    // Data Center has no "current user" REST resource; this servlet echoes the username.
    let name = get_text(auth, &format!("https://{}/plugins/servlet/applinks/whoami", account.host))?;
    let name = name.trim();
    if name.is_empty() || name.starts_with('<') {
        return Err(Error::Auth(format!("the credentials were not accepted by {}", account.host)));
    }
    Ok(name.to_owned())
}

// ---------------------------------------------------------------- cloud

#[derive(Deserialize)]
struct Branch {
    name: String,
}

#[derive(Deserialize, Default)]
struct Commit {
    #[serde(default)]
    hash: String,
}

#[derive(Deserialize)]
struct Endpoint {
    branch: Branch,
    #[serde(default)]
    commit: Option<Commit>,
}

#[derive(Deserialize)]
struct Href {
    href: String,
}

#[derive(Deserialize)]
struct Links {
    html: Href,
}

#[derive(Deserialize)]
struct Participant {
    user: CloudUser,
    #[serde(default)]
    role: String,
    #[serde(default)]
    approved: bool,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Deserialize)]
struct CloudPr {
    id: u64,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    draft: bool,
    source: Endpoint,
    destination: Endpoint,
    #[serde(default)]
    author: CloudUser,
    links: Links,
    #[serde(default)]
    participants: Vec<Participant>,
}

fn convert_cloud(p: CloudPr) -> PullRequest {
    let reviewers = p
        .participants
        .into_iter()
        .filter(|x| x.role == "REVIEWER" || x.approved || x.state.is_some())
        .map(|x| {
            let state = match (x.approved, x.state.as_deref()) {
                (true, _) | (_, Some("approved")) => "approved",
                (_, Some("changes_requested")) => "changes requested",
                _ => "no vote",
            };
            Reviewer { name: x.user.display_name, state: state.into(), required: false }
        })
        .collect();
    PullRequest {
        id: p.id,
        title: p.title,
        description: p.description,
        source: p.source.branch.name,
        target: p.destination.branch.name,
        author: p.author.display_name,
        draft: p.draft,
        reviewers,
        // Bitbucket Cloud reports a 12-character hash prefix.
        head: p.source.commit.map(|c| c.hash).filter(|h| !h.is_empty()),
        merge_status: None,
        url: p.links.html.href,
    }
}

// ---------------------------------------------------------------- data center

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DcRef {
    display_id: String,
    #[serde(default)]
    latest_commit: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DcUser {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
struct DcParticipant {
    user: DcUser,
    #[serde(default)]
    status: String,
}

#[derive(Deserialize)]
struct DcLinks {
    #[serde(rename = "self", default)]
    this: Vec<Href>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DcPr {
    id: u64,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    draft: bool,
    from_ref: DcRef,
    to_ref: DcRef,
    author: DcParticipant,
    #[serde(default)]
    reviewers: Vec<DcParticipant>,
    links: DcLinks,
}

fn convert_dc(p: DcPr) -> PullRequest {
    PullRequest {
        id: p.id,
        title: p.title,
        description: p.description,
        source: p.from_ref.display_id,
        target: p.to_ref.display_id,
        author: p.author.user.display_name,
        draft: p.draft,
        reviewers: p
            .reviewers
            .into_iter()
            .map(|r| {
                let state = match r.status.as_str() {
                    "APPROVED" => "approved",
                    "NEEDS_WORK" => "changes requested",
                    _ => "no vote",
                };
                Reviewer { name: r.user.display_name, state: state.into(), required: false }
            })
            .collect(),
        head: p.from_ref.latest_commit,
        merge_status: None,
        url: p.links.this.into_iter().next().map(|h| h.href).unwrap_or_default(),
    }
}

// ---------------------------------------------------------------- operations

pub fn pull_requests(t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
    if t.provider == Provider::Bitbucket {
        let url = format!("{}/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants", cloud_repo(t));
        Ok(cloud_list::<CloudPr>(auth, url)?.into_iter().map(convert_cloud).collect())
    } else {
        let url = format!("{}/pull-requests?state=OPEN&limit=100", dc_repo(t));
        Ok(dc_list::<DcPr>(auth, url)?.into_iter().map(convert_dc).collect())
    }
}

pub fn default_branch(t: &Target, auth: &Auth) -> Result<Option<String>> {
    if t.provider == Provider::Bitbucket {
        #[derive(Deserialize)]
        struct Repo {
            mainbranch: Option<Branch>,
        }
        return Ok(get::<Repo>(auth, &cloud_repo(t))?.mainbranch.map(|b| b.name));
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Default {
        display_id: String,
    }
    // /default-branch is Bitbucket 7.5+; older servers use /branches/default.
    let d = get::<Default>(auth, &format!("{}/default-branch", dc_repo(t)))
        .or_else(|_| get::<Default>(auth, &format!("{}/branches/default", dc_repo(t))))?;
    Ok(Some(d.display_id))
}

pub fn create_pull_request(t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
    if t.provider == Provider::Bitbucket {
        let body = json!({
            "title": pr.title,
            "description": pr.description,
            "draft": pr.draft,
            "source": { "branch": { "name": pr.source } },
            "destination": { "branch": { "name": pr.target } },
        });
        return Ok(convert_cloud(post(auth, &format!("{}/pullrequests", cloud_repo(t)), body)?));
    }
    let repo = json!({ "slug": t.repo, "project": { "key": t.owner } });
    let body = json!({
        "title": pr.title,
        "description": pr.description,
        "draft": pr.draft,
        "fromRef": { "id": format!("refs/heads/{}", pr.source), "repository": repo },
        "toRef": { "id": format!("refs/heads/{}", pr.target), "repository": repo },
    });
    Ok(convert_dc(post(auth, &format!("{}/pull-requests", dc_repo(t)), body)?))
}

#[derive(Deserialize)]
struct CloudIssue {
    id: u64,
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    assignee: Option<CloudUser>,
    links: Links,
}

fn to_issue(i: CloudIssue) -> Issue {
    Issue {
        key: i.id.to_string(),
        title: i.title,
        state: i.state,
        kind: i.kind,
        assignee: i.assignee.map(|a| a.display_name),
        url: i.links.html.href,
    }
}

/// Assigned open issues; repositories with the issue tracker turned off have none.
pub fn my_issues(t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
    let me: CloudUser = get(auth, &format!("{CLOUD}/user"))?;
    let q = format!(r#"assignee.uuid="{}" AND (state="new" OR state="open")"#, me.uuid);
    match cloud_list::<CloudIssue>(auth, format!("{}/issues?q={}&sort=-updated_on&pagelen=50", cloud_repo(t), enc(&q))) {
        Ok(items) => Ok(items.into_iter().map(to_issue).collect()),
        Err(e) if e.is_not_found() => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

pub fn issues(t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
    let mut out = Vec::new();
    for k in keys.iter().filter(|k| k.parse::<u64>().is_ok()).take(30) {
        match get::<CloudIssue>(auth, &format!("{}/issues/{k}", cloud_repo(t))) {
            Ok(i) => out.push(to_issue(i)),
            Err(e) if e.is_not_found() => {}
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// The Bitbucket [`Forge`](super::Forge).
pub struct Bitbucket;

impl super::Forge for Bitbucket {
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
        if t.provider == Provider::BitbucketDataCenter { Ok(Vec::new()) } else { my_issues(t, auth) }
    }
    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        if t.provider == Provider::BitbucketDataCenter { Ok(Vec::new()) } else { issues(t, auth, keys) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_cloud_and_dc() {
        let p: Values<CloudPr> = serde_json::from_str(
            r#"{"values":[{"id":12,"title":"Fix","description":"","source":{"branch":{"name":"7-fix"},"commit":{"hash":"0123456789ab"}},
            "destination":{"branch":{"name":"main"}},"author":{"display_name":"Ada","uuid":"{1}"},"links":{"html":{"href":"https://bitbucket.org/t/a/pull-requests/12"}},
            "participants":[{"user":{"display_name":"Grace"},"role":"REVIEWER","approved":true},{"user":{"display_name":"Bob"},"role":"PARTICIPANT","approved":false}]}]}"#,
        )
        .unwrap();
        let pr = convert_cloud(p.values.into_iter().next().unwrap());
        assert_eq!((pr.id, pr.source.as_str(), pr.approvals(), pr.reviewers.len()), (12, "7-fix", 1, 1));
        assert!(pr.head_matches("0123456789abcdef0000"));

        let d: Values<DcPr> = serde_json::from_str(
            r#"{"values":[{"id":3,"title":"Port","fromRef":{"displayId":"PROJ-1-port","latestCommit":"deadbeef00"},"toRef":{"displayId":"master"},
            "author":{"user":{"displayName":"Ada"},"status":"UNAPPROVED"},"reviewers":[{"user":{"displayName":"Linus"},"status":"NEEDS_WORK"}],
            "links":{"self":[{"href":"https://bb.acme.io/projects/P/repos/r/pull-requests/3"}]}}]}"#,
        )
        .unwrap();
        let pr = convert_dc(d.values.into_iter().next().unwrap());
        assert_eq!(
            (pr.source.as_str(), pr.target.as_str(), pr.reviewers[0].state.as_str()),
            ("PROJ-1-port", "master", "changes requested")
        );
        assert!(pr.url.ends_with("/pull-requests/3"));
    }
}

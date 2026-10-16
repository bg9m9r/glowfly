//! GitLab.com and GitLab Self-Managed (REST API v4).

use super::checks::per_pr;
use super::details::{self, PrDetails};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, CHECKED_PRS, Checks, Issue, NewPullRequest, PullRequest, Reviewer, Target, TipQuery, enc, enc_path, get, gitlab_checks,
    page, post,
};
use crate::error::Result;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub(super) static GITLAB: ProviderInfo =
    ProviderInfo { name: "GitLab", fixed_host: Some("gitlab.com"), host_hint: "", ..GITLAB_SELF_MANAGED };

pub(super) static GITLAB_SELF_MANAGED: ProviderInfo = ProviderInfo {
    name: "GitLab Self-Managed",
    host_hint: "e.g., gitlab.mycompany.com",
    token_help: "A personal access token with the api scope.",
    env_var: Some("GITLAB_TOKEN"),
    pr_noun: "merge request",
    pr_prefix: "!",
    close_keyword: Some("Closes"),
    lists_forks: true,
    creates_forks: true,
    adds_ssh_keys: true,
    details_per_pr: true,
    service: &GitLab,
    ..DEFAULTS
};

pub(super) fn project_api(t: &Target) -> String {
    format!("https://{}/api/v4/projects/{}", t.host, enc(&format!("{}/{}", t.owner, t.repo)))
}

#[derive(Deserialize)]
struct User {
    #[serde(default)]
    name: String,
    #[serde(default)]
    username: String,
}

#[derive(Deserialize)]
struct ApiMr {
    iid: u64,
    title: String,
    #[serde(default)]
    description: Option<String>,
    source_branch: String,
    target_branch: String,
    #[serde(default)]
    sha: Option<String>,
    author: User,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
    web_url: String,
    #[serde(default)]
    reviewers: Vec<User>,
    #[serde(default)]
    detailed_merge_status: Option<String>,
}

#[derive(Deserialize)]
struct Approvals {
    #[serde(default)]
    approved_by: Vec<ApprovedBy>,
}

#[derive(Deserialize)]
struct ApprovedBy {
    user: User,
}

/// A merge request with `votes` (its approvers) merged in the same way as the second pass.
fn convert(m: ApiMr, votes: Vec<Reviewer>, checks: Option<Checks>) -> PullRequest {
    let reviewers = m.reviewers.into_iter().map(|r| Reviewer { name: r.name, state: "no vote".into(), required: false }).collect();
    let mut pr = PullRequest {
        id: m.iid,
        title: m.title,
        description: m.description.unwrap_or_default(),
        source: m.source_branch,
        target: m.target_branch,
        author: m.author.name,
        draft: m.draft || m.work_in_progress,
        reviewers,
        head: m.sha,
        merge_status: m.detailed_merge_status,
        checks: None,
        url: m.web_url,
    };
    pr.apply(&PrDetails { checks, votes: Some(votes), ..PrDetails::empty(&pr) });
    pr
}

/// The merge request's approvers, as "approved" votes.
fn votes(t: &Target, auth: &Auth, iid: u64) -> Result<Vec<Reviewer>> {
    let a: Approvals = get(auth, &format!("{}/merge_requests/{iid}/approvals", project_api(t)))?;
    Ok(approvers(a))
}

fn approvers(a: Approvals) -> Vec<Reviewer> {
    a.approved_by.into_iter().map(|a| Reviewer { name: a.user.name, state: "approved".into(), required: false }).collect()
}

#[derive(Deserialize)]
struct ApiIssue {
    iid: u64,
    title: String,
    state: String,
    web_url: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    assignee: Option<User>,
    #[serde(default)]
    issue_type: Option<String>,
}

fn to_issue(i: ApiIssue) -> Issue {
    let kind = i.labels.into_iter().next().or(i.issue_type).unwrap_or_else(|| "issue".into());
    Issue { key: i.iid.to_string(), title: i.title, state: i.state, kind, assignee: i.assignee.map(|a| a.name), url: i.web_url }
}

/// The GitLab [`Forge`](super::Forge).
pub struct GitLab;

impl super::Forge for GitLab {
    fn issue_url(&self, t: &Target, key: &str) -> String {
        // A GitLab namespace can be several path segments.
        format!("https://{}/{}/{}/-/issues/{}", t.host, enc_path(&t.owner), enc(&t.repo), enc(key))
    }

    fn token_url(&self, account: &Account) -> Option<String> {
        Some(format!("https://{}/-/user_settings/personal_access_tokens", account.host))
    }

    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        let u: User = get(auth, &format!("https://{}/api/v4/user", account.host))?;
        Ok(if u.name.is_empty() { u.username } else { u.name })
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let mrs: Vec<ApiMr> = page::by_link(auth, format!("{}/merge_requests?state=opened&per_page=100", project_api(t)))?;
        let checks = per_pr(&mrs, |m| gitlab_checks::head_pipeline(t, auth, m.iid));
        let mut out = Vec::with_capacity(mrs.len());
        for (i, (m, checks)) in mrs.into_iter().zip(checks).enumerate() {
            // An approvals lookup that fails leaves the merge request without votes.
            let votes = if i < CHECKED_PRS { votes(t, auth, m.iid).unwrap_or_default() } else { Vec::new() };
            out.push(convert(m, votes, checks));
        }
        Ok(out)
    }

    /// Approvals are required (a refusal stops the pass); pipelines are skipped after the first
    /// refusal, so a token that can't read them still gets votes.
    fn pr_details(&self, t: &Target, auth: &Auth, prs: &[PullRequest]) -> Result<Vec<PrDetails>> {
        let mut ask_pipelines = true;
        details::each(prs, |p| {
            let votes = votes(t, auth, p.id)?;
            let checks = if ask_pipelines {
                gitlab_checks::head_pipeline(t, auth, p.id).unwrap_or_else(|e| {
                    ask_pipelines = e.is_not_found();
                    None
                })
            } else {
                None
            };
            Ok(PrDetails { checks, votes: Some(votes), ..PrDetails::empty(p) })
        })
    }

    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Project {
            default_branch: Option<String>,
        }
        Ok(get::<Project>(auth, &project_api(t))?.default_branch)
    }

    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        let title = if pr.draft { format!("Draft: {}", pr.title) } else { pr.title.clone() };
        let body = json!({
            "source_branch": pr.source,
            "target_branch": pr.target,
            "title": title,
            "description": pr.description,
            "remove_source_branch": false,
        });
        let created: ApiMr = post(auth, &format!("{}/merge_requests", project_api(t)), body)?;
        Ok(convert(created, Vec::new(), None))
    }

    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        let url = format!("{}/issues?state=opened&scope=assigned_to_me&per_page=100&order_by=updated_at", project_api(t));
        Ok(page::by_link::<ApiIssue>(auth, url)?.into_iter().map(to_issue).collect())
    }

    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        let iids: Vec<String> = keys.iter().filter(|k| k.parse::<u64>().is_ok()).map(|k| format!("iids[]={k}")).collect();
        if iids.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/issues?scope=all&per_page=100&{}", project_api(t), iids.join("&"));
        Ok(page::by_link::<ApiIssue>(auth, url)?.into_iter().map(to_issue).collect())
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
        gitlab_checks::commit_checks(t, auth, tips)
    }

    fn forks(&self, t: &Target, auth: &Auth) -> Result<super::ForkList> {
        super::forks::gitlab::list(t, auth)
    }

    fn my_fork(&self, t: &Target, auth: &Auth) -> Result<super::ForkLookup> {
        super::forks::gitlab::my_fork(t, auth)
    }

    fn create_fork(&self, t: &Target, auth: &Auth, wait: &super::Wait) -> Result<super::CreatedFork> {
        super::forks::gitlab::create(t, auth, wait)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::Provider;

    #[test]
    fn converts_merge_requests() {
        let t = Target {
            provider: Provider::GitLab,
            host: "gitlab.com".into(),
            owner: "group/sub".into(),
            project: String::new(),
            repo: "app".into(),
        };
        assert_eq!(project_api(&t), "https://gitlab.com/api/v4/projects/group%2Fsub%2Fapp");
        let m: ApiMr = serde_json::from_str(
            r#"{"iid":3,"title":"Draft: speed up","description":"Closes #2","source_branch":"2-speed","target_branch":"main",
            "sha":"0123456789abcdef","author":{"name":"Ada","username":"ada"},"draft":true,"web_url":"u",
            "reviewers":[{"name":"Grace","username":"g"},{"name":"Linus","username":"l"}],"detailed_merge_status":"mergeable"}"#,
        )
        .unwrap();
        let a: Approvals = serde_json::from_str(r#"{"approved_by":[{"user":{"name":"Grace","username":"g"}}]}"#).unwrap();
        let pr = convert(m, approvers(a), None);
        assert!(pr.draft);
        assert_eq!(pr.approvals(), 1);
        assert_eq!(pr.reviewers.len(), 2);
        let i: ApiIssue =
            serde_json::from_str(r#"{"iid":2,"title":"Slow","state":"opened","web_url":"u","labels":[],"issue_type":"issue"}"#).unwrap();
        assert_eq!(to_issue(i).key, "2");
    }
}

//! Bitbucket Cloud (API 2.0). Data Center / Server is in `bitbucket_dc`.

use super::checks::per_pr;
use super::details::{PrDetails, checks_of};
use super::issue_details::{CreateOptions, IssueDetails, NewIssue};
use super::page::{self, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{
    Account, Auth, Checks, Conditions, Entry, FilterKind, FilterOptions, Issue, IssueFacets, Me, NewPullRequest, PrFacets, PullRequest,
    Reviewer, Target, TipQuery, bitbucket_checks, bitbucket_issues, enc, get, post,
};
use crate::error::Result;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;

pub(super) static BITBUCKET: ProviderInfo = ProviderInfo {
    name: "Bitbucket",
    fixed_host: Some("bitbucket.org"),
    username_label: Some("Username"),
    token_help: "Your Bitbucket username and an app password with Repositories, Pull requests and Issues (Read and Write to create issues) permissions, and Workspace membership (Read) for assignees.",
    close_keyword: Some("Resolves"),
    lists_forks: true,
    adds_ssh_keys: true,
    details_per_pr: true,
    issues_in_app: true,
    service: &Bitbucket,
    ..DEFAULTS
};

pub(super) const CLOUD: &str = "https://api.bitbucket.org/2.0";

pub(super) fn cloud_repo(t: &Target) -> String {
    format!("{CLOUD}/repositories/{}/{}", enc(&t.owner), enc(&t.repo))
}

/// A page of a list: Cloud links the next page; Data Center gives the next start index.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Values<T> {
    pub values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    pub is_last_page: Option<bool>,
    #[serde(default)]
    pub next_page_start: Option<u64>,
}

pub(super) fn cloud_list<T: DeserializeOwned>(auth: &Auth, first: String) -> Result<Vec<T>> {
    page::collect(first, |url| {
        let v: Values<T> = get(auth, url)?;
        Ok(Page { items: v.values, next: v.next })
    })
}

#[derive(Deserialize, Default)]
pub(super) struct CloudUser {
    #[serde(default)]
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) uuid: String,
}

#[derive(Deserialize)]
pub(super) struct Name {
    pub(super) name: String,
}

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
pub(super) struct Href {
    pub href: String,
}

#[derive(Deserialize)]
pub(super) struct Links {
    pub(super) html: Href,
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

fn convert_cloud(p: CloudPr, checks: Option<Checks>) -> PullRequest {
    let asked: Vec<&Participant> = p.participants.iter().filter(|x| x.role == "REVIEWER").collect();
    let facets = PrFacets {
        author_id: p.author.uuid.clone(),
        reviewer_ids: asked.iter().map(|x| x.user.uuid.clone()).collect(),
        awaiting_ids: asked.iter().filter(|x| !x.approved && x.state.is_none()).map(|x| x.user.uuid.clone()).collect(),
        ..PrFacets::default()
    };
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
        checks,
        url: p.links.html.href,
        facets,
    }
}

#[derive(Deserialize)]
pub(super) struct CloudIssue {
    id: u64,
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    assignee: Option<CloudUser>,
    links: Links,
    #[serde(default)]
    milestone: Option<Name>,
}

pub(super) fn to_issue(i: CloudIssue) -> Issue {
    Issue {
        key: i.id.to_string(),
        title: i.title,
        state: i.state,
        kind: i.kind,
        assignee: i.assignee.map(|a| a.display_name),
        url: i.links.html.href,
        facets: IssueFacets { labels: Vec::new(), milestone: i.milestone.map(|m| m.name) },
    }
}

/// The Bitbucket Cloud [`Forge`](super::Forge).
pub struct Bitbucket;

impl super::Forge for Bitbucket {
    fn verify(&self, _account: &Account, auth: &Auth) -> Result<String> {
        Ok(get::<CloudUser>(auth, &format!("{CLOUD}/user"))?.display_name)
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let url = format!("{}/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants", cloud_repo(t));
        let prs = cloud_list::<CloudPr>(auth, url)?;
        let checks = per_pr(&prs, |p| bitbucket_checks::cloud_pr(t, auth, p.id));
        Ok(prs.into_iter().zip(checks).map(|(p, c)| convert_cloud(p, c)).collect())
    }

    fn pr_details(&self, t: &Target, auth: &Auth, prs: &[PullRequest]) -> Result<Vec<PrDetails>> {
        checks_of(prs, |p| bitbucket_checks::cloud_pr(t, auth, p.id))
    }

    fn check_list(&self, t: &Target, auth: &Auth, q: &super::CheckQuery) -> Result<super::CheckList> {
        bitbucket_checks::cloud_list(t, auth, q.id)
    }

    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Repo {
            mainbranch: Option<Branch>,
        }
        Ok(get::<Repo>(auth, &cloud_repo(t))?.mainbranch.map(|b| b.name))
    }

    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        let body = json!({
            "title": pr.title,
            "description": pr.description,
            "draft": pr.draft,
            "source": { "branch": { "name": pr.source } },
            "destination": { "branch": { "name": pr.target } },
        });
        Ok(convert_cloud(post(auth, &format!("{}/pullrequests", cloud_repo(t)), body)?, None))
    }

    /// Assigned open issues; repositories with the issue tracker turned off have none.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        let me: CloudUser = get(auth, &format!("{CLOUD}/user"))?;
        let q = format!(r#"assignee.uuid="{}" AND (state="new" OR state="open")"#, me.uuid);
        match cloud_list::<CloudIssue>(auth, format!("{}/issues?q={}&sort=-updated_on&pagelen=50", cloud_repo(t), enc(&q))) {
            Ok(items) => Ok(items.into_iter().map(to_issue).collect()),
            Err(e) if e.is_not_found() => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
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

    fn issue_url(&self, t: &Target, key: &str) -> String {
        format!("https://bitbucket.org/{}/{}/issues/{}", enc(&t.owner), enc(&t.repo), enc(key))
    }

    fn token_url(&self, _account: &Account) -> Option<String> {
        Some("https://bitbucket.org/account/settings/app-passwords/".into())
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
        bitbucket_checks::cloud_commits(t, auth, tips)
    }

    fn forks(&self, t: &Target, auth: &Auth) -> Result<super::ForkList> {
        super::forks::bitbucket::cloud(t, auth)
    }

    fn me(&self, _t: &Target, auth: &Auth) -> Result<Me> {
        super::filters::bitbucket::me(auth)
    }

    fn find_issues(&self, t: &Target, auth: &Auth, c: &Conditions, me: Option<&Me>) -> Result<Vec<Issue>> {
        super::filters::bitbucket::find_issues(t, auth, c, me)
    }

    fn filter_options(&self, t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
        super::filters::bitbucket::options(t, auth, kind)
    }

    fn issue_details(&self, t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
        bitbucket_issues::details(t, auth, key)
    }

    fn issue_create_options(&self, t: &Target, auth: &Auth, _project: &str) -> Result<CreateOptions> {
        bitbucket_issues::options(t, auth)
    }

    fn create_issue(&self, t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
        bitbucket_issues::create(t, auth, new)
    }

    fn post_issue_comment(&self, t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
        bitbucket_issues::post_comment(t, auth, key, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_cloud_pull_requests() {
        let p: Values<CloudPr> = serde_json::from_str(
            r#"{"values":[{"id":12,"title":"Fix","description":"","source":{"branch":{"name":"7-fix"},"commit":{"hash":"0123456789ab"}},
            "destination":{"branch":{"name":"main"}},"author":{"display_name":"Ada","uuid":"{1}"},"links":{"html":{"href":"https://bitbucket.org/t/a/pull-requests/12"}},
            "participants":[{"user":{"display_name":"Grace"},"role":"REVIEWER","approved":true},{"user":{"display_name":"Bob"},"role":"PARTICIPANT","approved":false}]}]}"#,
        )
        .unwrap();
        let pr = convert_cloud(p.values.into_iter().next().unwrap(), None);
        assert_eq!((pr.id, pr.source.as_str(), pr.approvals(), pr.reviewers.len()), (12, "7-fix", 1, 1));
        assert!(pr.head_matches("0123456789abcdef0000"));
    }

    #[test]
    fn cloud_facets_use_account_uuids() {
        let p: CloudPr = serde_json::from_str(
            r#"{"id":1,"title":"T","source":{"branch":{"name":"x"}},"destination":{"branch":{"name":"main"}},
            "author":{"display_name":"Ada","uuid":"{a}"},"links":{"html":{"href":"u"}},
            "participants":[{"user":{"uuid":"{g}"},"role":"REVIEWER","approved":true},{"user":{"uuid":"{l}"},"role":"REVIEWER"},
            {"user":{"uuid":"{b}"},"role":"PARTICIPANT"}]}"#,
        )
        .unwrap();
        let f = convert_cloud(p, None).facets;
        assert_eq!((f.author_id.as_str(), f.reviewer_ids.len(), f.awaiting_ids), ("{a}", 2, vec!["{l}".to_owned()]));
        let i: CloudIssue =
            serde_json::from_str(r#"{"id":3,"title":"C","links":{"html":{"href":"u"}},"milestone":{"name":"m1"}}"#).unwrap();
        assert_eq!(to_issue(i).facets.milestone.as_deref(), Some("m1"));
    }
}

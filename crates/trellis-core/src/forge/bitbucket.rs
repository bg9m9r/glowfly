//! Bitbucket Cloud (API 2.0). Data Center / Server is in `bitbucket_dc`.

use super::page::{self, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{Account, Auth, Issue, NewPullRequest, PullRequest, Reviewer, Target, enc, get, post};
use crate::error::Result;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

pub(super) static BITBUCKET: ProviderInfo = ProviderInfo {
    name: "Bitbucket",
    fixed_host: Some("bitbucket.org"),
    username_label: Some("Username"),
    token_help: "Your Bitbucket username and an app password with Repositories, Pull requests and Issues permissions.",
    close_keyword: Some("Resolves"),
    lists_forks: true,
    service: &Bitbucket,
    ..DEFAULTS
};

const CLOUD: &str = "https://api.bitbucket.org/2.0";

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
struct CloudUser {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    uuid: String,
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

/// The Bitbucket Cloud [`Forge`](super::Forge).
pub struct Bitbucket;

impl super::Forge for Bitbucket {
    fn verify(&self, _account: &Account, auth: &Auth) -> Result<String> {
        Ok(get::<CloudUser>(auth, &format!("{CLOUD}/user"))?.display_name)
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let url = format!("{}/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants", cloud_repo(t));
        Ok(cloud_list::<CloudPr>(auth, url)?.into_iter().map(convert_cloud).collect())
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
        Ok(convert_cloud(post(auth, &format!("{}/pullrequests", cloud_repo(t)), body)?))
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

    fn forks(&self, t: &Target, auth: &Auth) -> Result<super::ForkList> {
        super::forks::bitbucket::cloud(t, auth)
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
        let pr = convert_cloud(p.values.into_iter().next().unwrap());
        assert_eq!((pr.id, pr.source.as_str(), pr.approvals(), pr.reviewers.len()), (12, "7-fix", 1, 1));
        assert!(pr.head_matches("0123456789abcdef0000"));
    }
}

//! Bitbucket Data Center / Server (REST 1.0). It has no issue tracker.

use super::bitbucket::{Href, Values};
use super::checks::{full_sha, per_pr};
use super::details::{PrDetails, checks_of};
use super::page::{self, Page};
use super::provider::{DEFAULTS, ProviderInfo};
use super::{Account, Auth, Checks, NewPullRequest, PullRequest, Reviewer, Target, TipQuery, bitbucket_checks, enc, get, get_text, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;

pub(super) static BITBUCKET_DATA_CENTER: ProviderInfo = ProviderInfo {
    name: "Bitbucket Data Center",
    host_hint: "e.g., bitbucket.mycompany.com",
    token_help: "An HTTP access token with Repository write permission.",
    has_issues: false,
    lists_forks: true,
    details_per_pr: true,
    service: &BitbucketDc,
    ..DEFAULTS
};

pub(super) fn dc_repo(t: &Target) -> String {
    format!("https://{}/rest/api/1.0/projects/{}/repos/{}", t.host, enc(&t.owner), enc(&t.repo))
}

pub(super) fn dc_list<T: DeserializeOwned>(auth: &Auth, first: String) -> Result<Vec<T>> {
    page::collect(first.clone(), |url| {
        let v: Values<T> = get(auth, url)?;
        let next = match (v.is_last_page, v.next_page_start) {
            (Some(false), Some(start)) => Some(page::with_param(&first, "start", &start.to_string())),
            _ => None,
        };
        Ok(Page { items: v.values, next })
    })
}

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

fn convert_dc(p: DcPr, checks: Option<Checks>) -> PullRequest {
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
        checks,
        url: p.links.this.into_iter().next().map(|h| h.href).unwrap_or_default(),
    }
}

/// The Bitbucket Data Center [`Forge`](super::Forge).
pub struct BitbucketDc;

impl super::Forge for BitbucketDc {
    fn verify(&self, account: &Account, auth: &Auth) -> Result<String> {
        // Data Center has no "current user" REST resource; this servlet echoes the username.
        let name = get_text(auth, &format!("https://{}/plugins/servlet/applinks/whoami", account.host))?;
        let name = name.trim();
        if name.is_empty() || name.starts_with('<') {
            return Err(Error::Auth(format!("the credentials were not accepted by {}", account.host)));
        }
        Ok(name.to_owned())
    }

    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
        let url = format!("{}/pull-requests?state=OPEN&limit=100", dc_repo(t));
        let prs = dc_list::<DcPr>(auth, url)?;
        let checks = per_pr(&prs, |p| match p.from_ref.latest_commit.as_deref().filter(|c| full_sha(c)) {
            Some(sha) => bitbucket_checks::dc_commit(auth, &t.host, sha),
            None => Ok(None),
        });
        Ok(prs.into_iter().zip(checks).map(|(p, c)| convert_dc(p, c)).collect())
    }

    /// Build statuses of each head commit; a pull request without a full commit id has none.
    fn pr_details(&self, t: &Target, auth: &Auth, prs: &[PullRequest]) -> Result<Vec<PrDetails>> {
        checks_of(prs, |p| match p.head.as_deref().filter(|c| full_sha(c)) {
            Some(sha) => bitbucket_checks::dc_commit(auth, &t.host, sha),
            None => Ok(None),
        })
    }

    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>> {
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

    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
        let repo = json!({ "slug": t.repo, "project": { "key": t.owner } });
        let body = json!({
            "title": pr.title,
            "description": pr.description,
            "draft": pr.draft,
            "fromRef": { "id": format!("refs/heads/{}", pr.source), "repository": repo },
            "toRef": { "id": format!("refs/heads/{}", pr.target), "repository": repo },
        });
        Ok(convert_dc(post(auth, &format!("{}/pull-requests", dc_repo(t)), body)?, None))
    }

    fn token_url(&self, account: &Account) -> Option<String> {
        Some(format!("https://{}/plugins/servlet/access-tokens/manage", account.host))
    }

    /// `~user` owners are personal repositories, under /users/.
    fn repo_url(&self, t: &Target) -> String {
        match t.owner.strip_prefix('~') {
            Some(user) => format!("https://{}/users/{}/repos/{}/browse", t.host, enc(user), enc(&t.repo)),
            None => format!("https://{}/projects/{}/repos/{}/browse", t.host, enc(&t.owner), enc(&t.repo)),
        }
    }

    fn commit_checks(&self, t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
        bitbucket_checks::dc_commits(t, auth, tips)
    }

    fn forks(&self, t: &Target, auth: &Auth) -> Result<super::ForkList> {
        super::forks::bitbucket::data_center(t, auth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_data_center_pull_requests() {
        let d: Values<DcPr> = serde_json::from_str(
            r#"{"values":[{"id":3,"title":"Port","fromRef":{"displayId":"PROJ-1-port","latestCommit":"deadbeef00"},"toRef":{"displayId":"master"},
            "author":{"user":{"displayName":"Ada"},"status":"UNAPPROVED"},"reviewers":[{"user":{"displayName":"Linus"},"status":"NEEDS_WORK"}],
            "links":{"self":[{"href":"https://bb.acme.io/projects/P/repos/r/pull-requests/3"}]}}]}"#,
        )
        .unwrap();
        let pr = convert_dc(d.values.into_iter().next().unwrap(), None);
        assert_eq!(
            (pr.source.as_str(), pr.target.as_str(), pr.reviewers[0].state.as_str()),
            ("PROJ-1-port", "master", "changes requested")
        );
        assert!(pr.url.ends_with("/pull-requests/3"));
    }
}

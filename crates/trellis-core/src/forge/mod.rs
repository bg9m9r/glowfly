//! Hosting-service integrations ("forges"): remote detection, credentials, pull/merge requests,
//! issues, forks, the user's own repositories and repository web pages for GitHub, GitLab,
//! Bitbucket, Azure DevOps and Jira, behind one common model.

mod azure;
mod bitbucket;
mod bitbucket_dc;
mod github;
mod gitlab;
mod jira;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

mod auth;
mod forks;
mod http;
mod links;
mod page;
mod provider;
mod publish;
mod refs;
mod remote;
mod user_repos;
mod web;

pub use auth::{
    Auth, delete_token, forget_credential_manager, from_credential_manager, is_connected, is_public_host, may_contact, resolve_auth,
    save_token,
};
pub use forks::{CreatedFork, Fork, ForkList, ForkLookup, Wait, create_fork, find_my_fork, list_forks, remote_name_for, same_repository};
pub(crate) use http::read_capped;
use http::{dec, enc, enc_path, get, get_text, post};
pub use links::https_host;
pub use provider::{Provider, ProviderInfo};
pub use publish::{CreatedRepo, NewRepo, create_repo, repo_name_for, repo_owners, valid_repo_name};
pub use refs::{RefStyle, branch_issue, branch_name_for, issue_refs, mention};
pub use remote::{Connection, parse_host, parse_remote};
pub use user_repos::{HostedRepo, MyRepos, lists_my_repos, my_repos};
pub use web::{NoWebPage, WebPage, web_page};

/// A connection identity: which service, which host, and (Azure DevOps) which organization.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Account {
    pub provider: Provider,
    pub host: String,
    #[serde(default)]
    pub org: String,
}

impl Account {
    pub fn key(&self) -> String {
        format!("{:?}:{}/{}", self.provider, self.host, self.org)
    }

    pub fn display(&self) -> String {
        if self.org.is_empty() { self.host.clone() } else { format!("{}/{}", self.host, self.org) }
    }
}

/// A repository (or, for Jira, a site) on a hosting service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub provider: Provider,
    /// Web host, e.g. github.com, gitlab.mycompany.com, dev.azure.com, site.atlassian.net.
    pub host: String,
    /// Owner / namespace / workspace / project key / Azure organization.
    pub owner: String,
    /// Azure DevOps project, or the Jira project filter.
    pub project: String,
    pub repo: String,
}

impl Target {
    pub fn account(&self) -> Account {
        let org = if self.provider == Provider::AzureDevOps { self.owner.clone() } else { String::new() };
        Account { provider: self.provider, host: self.host.clone(), org }
    }

    pub fn jira(account: &Account, project: &str) -> Target {
        Target {
            provider: account.provider,
            host: account.host.clone(),
            owner: String::new(),
            project: project.to_owned(),
            repo: String::new(),
        }
    }

    pub fn display(&self) -> String {
        self.provider.info().service.display(self)
    }

    /// Web page for an issue; empty when the service has no issue tracker.
    pub fn issue_url(&self, key: &str) -> String {
        self.provider.info().service.issue_url(self, key)
    }

    /// Link to create a personal access token for this service.
    pub fn token_url(account: &Account) -> Option<String> {
        account.provider.info().service.token_url(account)
    }
}

// -------------------------------------------------------------------- common model

#[derive(Clone, Debug)]
pub struct Reviewer {
    pub name: String,
    /// approved, changes requested, waiting, rejected, no vote…
    pub state: String,
    pub required: bool,
}

#[derive(Clone, Debug)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub source: String,
    pub target: String,
    pub author: String,
    pub draft: bool,
    pub reviewers: Vec<Reviewer>,
    /// Head commit SHA (Bitbucket Cloud reports only a 12-character prefix).
    pub head: Option<String>,
    pub merge_status: Option<String>,
    /// Web page; empty when the service sent no usable link.
    pub url: String,
}

impl PullRequest {
    pub fn approvals(&self) -> usize {
        self.reviewers.iter().filter(|r| r.state.starts_with("approved")).count()
    }

    pub fn head_matches(&self, sha: &str) -> bool {
        self.head.as_deref().is_some_and(|h| h.len() >= 7 && sha.starts_with(h))
    }
}

#[derive(Clone, Debug)]
pub struct Issue {
    /// Display key: `123` for numbered trackers, `PROJ-123` for Jira.
    pub key: String,
    pub title: String,
    pub state: String,
    pub kind: String,
    pub assignee: Option<String>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewPullRequest {
    pub source: String,
    pub target: String,
    pub title: String,
    pub description: String,
    pub draft: bool,
    /// Issue keys to link.
    pub issues: Vec<String>,
}

// -------------------------------------------------------------------- service trait

/// One hosting service's API. Code hosts implement everything; the defaults suit an issue
/// tracker (Jira) that has no repositories or pull requests.
pub trait Forge: Sync {
    /// Checks credentials and returns the signed-in user's display name.
    fn verify(&self, account: &Account, auth: &Auth) -> Result<String>;

    fn pull_requests(&self, _t: &Target, _auth: &Auth) -> Result<Vec<PullRequest>> {
        Ok(Vec::new())
    }

    fn default_branch(&self, _t: &Target, _auth: &Auth) -> Result<Option<String>> {
        Ok(None)
    }

    fn create_pull_request(&self, _t: &Target, _auth: &Auth, _pr: &NewPullRequest) -> Result<PullRequest> {
        Err(Error::Unsupported("this service does not host pull requests".into()))
    }

    /// Open issues assigned to the signed-in user.
    fn my_issues(&self, _t: &Target, _auth: &Auth) -> Result<Vec<Issue>> {
        Ok(Vec::new())
    }

    /// Looks up specific issues by key; unknown keys are skipped.
    fn issues(&self, _t: &Target, _auth: &Auth, _keys: &[String]) -> Result<Vec<Issue>> {
        Ok(Vec::new())
    }

    /// Web page for an issue (the key is not yet encoded); empty without an issue tracker.
    fn issue_url(&self, _t: &Target, _key: &str) -> String {
        String::new()
    }

    /// Where to create a personal access token.
    fn token_url(&self, account: &Account) -> Option<String>;

    /// How a repository is named in the interface.
    fn display(&self, t: &Target) -> String {
        format!("{}/{}", t.owner, t.repo)
    }

    /// The repository's home page (not yet checked by `links::web_url`).
    fn repo_url(&self, t: &Target) -> String {
        format!("https://{}/{}/{}", t.host, enc_path(&t.owner), enc(&t.repo))
    }

    /// Forks of this repository, newest first, up to `page::MAX_ITEMS`.
    fn forks(&self, _t: &Target, _auth: &Auth) -> Result<ForkList> {
        Err(Error::Unsupported("listing forks isn't supported on this service".into()))
    }

    /// The signed-in user and the fork of this repository they already have.
    fn my_fork(&self, _t: &Target, _auth: &Auth) -> Result<ForkLookup> {
        Err(Error::Unsupported("finding your fork isn't supported on this service".into()))
    }

    /// Forks this repository into the signed-in user's account, waiting (per `wait`) until it
    /// is ready.
    fn create_fork(&self, _t: &Target, _auth: &Auth, _wait: &Wait) -> Result<CreatedFork> {
        Err(Error::Unsupported("forking isn't supported on this service".into()))
    }
}

/// The implementation for a provider.
pub fn service(provider: Provider) -> &'static dyn Forge {
    provider.info().service
}

pub fn verify(account: &Account, auth: &Auth) -> Result<String> {
    service(account.provider).verify(account, auth)
}

// Links in results are checked here, once for every service (see `links`).
pub fn pull_requests(t: &Target, auth: &Auth) -> Result<Vec<PullRequest>> {
    service(t.provider).pull_requests(t, auth).map(|prs| links::prs(t, prs))
}

pub fn default_branch(t: &Target, auth: &Auth) -> Result<Option<String>> {
    service(t.provider).default_branch(t, auth)
}

pub fn create_pull_request(t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest> {
    service(t.provider).create_pull_request(t, auth, pr).map(|p| links::pr(t, p))
}

pub fn my_issues(t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
    service(t.provider).my_issues(t, auth).map(|i| links::issues(t, i))
}

pub fn issues(t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    service(t.provider).issues(t, auth, keys).map(|i| links::issues(t, i))
}

/// Text appended to a PR description to link an issue, for trackers linked by mention.
pub fn link_line(code_host: Provider, tracker: Provider, key: &str) -> Option<String> {
    let host = code_host.info();
    if tracker.is_jira() {
        Some(key.to_owned())
    } else if code_host == tracker && host.links_issues_by_api {
        // Linked through the API when the pull request is created instead.
        None
    } else if let Some(word) = host.close_keyword {
        Some(format!("{word} #{key}"))
    } else {
        Some(mention(tracker.ref_style(), key))
    }
}

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

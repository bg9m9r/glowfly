//! Hosting-service integrations ("forges"): remote detection, credentials, pull/merge requests
//! and issues for GitHub, GitLab, Bitbucket, Azure DevOps and Jira, behind one common model.

mod azure;
mod bitbucket;
mod github;
mod gitlab;
mod jira;

use crate::error::Result;
use serde::{Deserialize, Serialize};

mod auth;
mod http;
mod links;
mod publish;
mod refs;
mod remote;

pub use auth::{
    Auth, delete_token, forget_credential_manager, from_credential_manager, is_connected, is_public_host, may_contact, resolve_auth,
    save_token,
};
use http::{dec, enc, get, get_text, post};
pub use links::https_host;
pub use publish::{CreatedRepo, NewRepo, create_repo, repo_name_for, repo_owners, valid_repo_name};
pub use refs::{RefStyle, branch_issue, branch_name_for, issue_refs, mention};
pub use remote::{Connection, parse_host, parse_remote};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    GitHub,
    GitHubEnterprise,
    GitLab,
    GitLabSelfManaged,
    Bitbucket,
    BitbucketDataCenter,
    AzureDevOps,
    JiraCloud,
    JiraDataCenter,
}

impl Provider {
    /// In the order Preferences lists them.
    pub const ALL: [Provider; 9] = [
        Provider::GitHub,
        Provider::GitHubEnterprise,
        Provider::GitLab,
        Provider::GitLabSelfManaged,
        Provider::Bitbucket,
        Provider::BitbucketDataCenter,
        Provider::AzureDevOps,
        Provider::JiraCloud,
        Provider::JiraDataCenter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Provider::GitHub => "GitHub",
            Provider::GitHubEnterprise => "GitHub Enterprise Server",
            Provider::GitLab => "GitLab",
            Provider::GitLabSelfManaged => "GitLab Self-Managed",
            Provider::Bitbucket => "Bitbucket",
            Provider::BitbucketDataCenter => "Bitbucket Data Center",
            Provider::AzureDevOps => "Azure DevOps",
            Provider::JiraCloud => "Jira Cloud",
            Provider::JiraDataCenter => "Jira Data Center",
        }
    }

    pub fn is_jira(self) -> bool {
        matches!(self, Provider::JiraCloud | Provider::JiraDataCenter)
    }

    /// Public SaaS host, for providers that have exactly one.
    pub fn fixed_host(self) -> Option<&'static str> {
        match self {
            Provider::GitHub => Some("github.com"),
            Provider::GitLab => Some("gitlab.com"),
            Provider::Bitbucket => Some("bitbucket.org"),
            _ => None,
        }
    }

    /// Git Credential Manager can sign in to every code host (not Jira).
    pub fn supports_gcm(self) -> bool {
        !self.is_jira()
    }

    /// Label for an extra username field, when the token must be paired with one.
    pub fn username_label(self) -> Option<&'static str> {
        match self {
            Provider::Bitbucket => Some("Username"),
            Provider::JiraCloud => Some("Email"),
            _ => None,
        }
    }

    pub fn host_hint(self) -> &'static str {
        match self {
            Provider::GitHubEnterprise => "e.g., github.mycompany.com",
            Provider::GitLabSelfManaged => "e.g., gitlab.mycompany.com",
            Provider::BitbucketDataCenter => "e.g., bitbucket.mycompany.com",
            Provider::AzureDevOps => "e.g., dev.azure.com/mycompany",
            Provider::JiraCloud => "e.g., mycompany.atlassian.net",
            Provider::JiraDataCenter => "e.g., jira.mycompany.com",
            _ => "",
        }
    }

    pub fn token_help(self) -> &'static str {
        match self {
            Provider::GitHub | Provider::GitHubEnterprise => {
                "A personal access token with the repo scope (classic), or a fine-grained token with Pull requests and Issues read & write."
            }
            Provider::GitLab | Provider::GitLabSelfManaged => "A personal access token with the api scope.",
            Provider::Bitbucket => "Your Bitbucket username and an app password with Repositories, Pull requests and Issues permissions.",
            Provider::BitbucketDataCenter => "An HTTP access token with Repository write permission.",
            Provider::AzureDevOps => "Needs the Code (Read & write) and Work Items (Read) scopes.",
            Provider::JiraCloud => "Your Atlassian account email and an API token.",
            Provider::JiraDataCenter => "A personal access token.",
        }
    }

    /// Environment variable checked before saved credentials (on the public host only).
    pub fn env_var(self) -> Option<&'static str> {
        match self {
            Provider::GitHub | Provider::GitHubEnterprise => Some("GITHUB_TOKEN"),
            Provider::GitLab | Provider::GitLabSelfManaged => Some("GITLAB_TOKEN"),
            Provider::AzureDevOps => Some("AZURE_DEVOPS_EXT_PAT"),
            _ => None,
        }
    }

    /// "Pull Request" or GitLab's "Merge Request".
    pub fn pr_noun(self) -> &'static str {
        if matches!(self, Provider::GitLab | Provider::GitLabSelfManaged) { "merge request" } else { "pull request" }
    }

    pub fn pr_prefix(self) -> &'static str {
        match self {
            Provider::GitLab | Provider::GitLabSelfManaged | Provider::AzureDevOps => "!",
            _ => "#",
        }
    }

    pub fn ref_style(self) -> RefStyle {
        match self {
            Provider::AzureDevOps => RefStyle::AzureBoards,
            Provider::JiraCloud | Provider::JiraDataCenter => RefStyle::Jira,
            _ => RefStyle::Hash,
        }
    }

    pub fn has_issues(self) -> bool {
        !matches!(self, Provider::BitbucketDataCenter)
    }
}

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
        match self.provider {
            Provider::AzureDevOps => format!("{}/{}/{}", self.owner, self.project, self.repo),
            Provider::JiraCloud | Provider::JiraDataCenter => {
                if self.project.is_empty() {
                    self.host.clone()
                } else {
                    format!("{} · {}", self.host, self.project)
                }
            }
            _ => format!("{}/{}", self.owner, self.repo),
        }
    }

    pub fn issue_url(&self, key: &str) -> String {
        let (h, o, r) = (&self.host, &self.owner, &self.repo);
        match self.provider {
            Provider::GitHub | Provider::GitHubEnterprise => format!("https://{h}/{o}/{r}/issues/{key}"),
            Provider::GitLab | Provider::GitLabSelfManaged => format!("https://{h}/{o}/{r}/-/issues/{key}"),
            Provider::Bitbucket => format!("https://bitbucket.org/{o}/{r}/issues/{key}"),
            Provider::BitbucketDataCenter => String::new(),
            Provider::AzureDevOps => azure::work_item_url(self, key),
            Provider::JiraCloud | Provider::JiraDataCenter => format!("https://{h}/browse/{key}"),
        }
    }

    /// Link to create a personal access token for this service.
    pub fn token_url(account: &Account) -> Option<String> {
        let h = &account.host;
        Some(match account.provider {
            Provider::GitHub | Provider::GitHubEnterprise => format!("https://{h}/settings/tokens"),
            Provider::GitLab | Provider::GitLabSelfManaged => format!("https://{h}/-/user_settings/personal_access_tokens"),
            Provider::Bitbucket => "https://bitbucket.org/account/settings/app-passwords/".into(),
            Provider::BitbucketDataCenter => format!("https://{h}/plugins/servlet/access-tokens/manage"),
            Provider::AzureDevOps if !account.org.is_empty() => {
                format!("https://dev.azure.com/{}/_usersSettings/tokens", enc(&account.org))
            }
            Provider::JiraCloud => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
            Provider::JiraDataCenter => format!("https://{h}/secure/ViewProfile.jspa"),
            _ => return None,
        })
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

/// One hosting service's API. Code hosts implement everything; issue trackers (Jira) return
/// empty results for pull requests.
pub trait Forge: Sync {
    /// Checks credentials and returns the signed-in user's display name.
    fn verify(&self, account: &Account, auth: &Auth) -> Result<String>;
    fn pull_requests(&self, t: &Target, auth: &Auth) -> Result<Vec<PullRequest>>;
    fn default_branch(&self, t: &Target, auth: &Auth) -> Result<Option<String>>;
    fn create_pull_request(&self, t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<PullRequest>;
    /// Open issues assigned to the signed-in user.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>>;
    /// Looks up specific issues by key; unknown keys are skipped.
    fn issues(&self, t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>>;
}

/// The implementation for a provider.
pub fn service(provider: Provider) -> &'static dyn Forge {
    match provider {
        Provider::GitHub | Provider::GitHubEnterprise => &github::GitHub,
        Provider::GitLab | Provider::GitLabSelfManaged => &gitlab::GitLab,
        Provider::Bitbucket | Provider::BitbucketDataCenter => &bitbucket::Bitbucket,
        Provider::AzureDevOps => &azure::AzureDevOps,
        Provider::JiraCloud | Provider::JiraDataCenter => &jira::Jira,
    }
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
    match (code_host, tracker) {
        // Azure DevOps links work items through the API instead.
        (Provider::AzureDevOps, Provider::AzureDevOps) => None,
        (_, Provider::JiraCloud | Provider::JiraDataCenter) => Some(key.to_owned()),
        (Provider::GitHub | Provider::GitHubEnterprise | Provider::GitLab | Provider::GitLabSelfManaged, _) => {
            Some(format!("Closes #{key}"))
        }
        (Provider::Bitbucket, _) => Some(format!("Resolves #{key}")),
        _ => Some(mention(tracker.ref_style(), key)),
    }
}

#[cfg(test)]
mod tests;

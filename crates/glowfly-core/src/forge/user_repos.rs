//! The signed-in user's own repositories on a hosting service (theirs, ones they collaborate on
//! and their organizations' or groups'), newest activity first, for the Clone dialog. Every
//! repository passes the same check as forks (`forks::checked`) before Glowfly shows or clones it.

use super::forks::{Fork, checked};
use super::{Account, Auth, Provider, Target, page};
use crate::error::{Error, Result};

mod bitbucket;
mod github;
mod gitlab;

/// A repository on a hosting service: the same shape as a fork (owner, `owner/name`, clone URLs,
/// web page, private).
pub type HostedRepo = Fork;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct MyRepos {
    /// Newest activity first.
    pub repos: Vec<HostedRepo>,
    /// The service had more than Glowfly lists (`page::MAX_ITEMS`).
    pub truncated: bool,
}

/// Whether Glowfly can list the user's repositories on `p`.
pub fn lists_my_repos(p: Provider) -> bool {
    matches!(p, Provider::GitHub | Provider::GitHubEnterprise | Provider::GitLab | Provider::GitLabSelfManaged | Provider::Bitbucket)
}

/// The repositories the user signed in to `account` can reach.
pub fn my_repos(account: &Account, auth: &Auth) -> Result<MyRepos> {
    let list = match account.provider {
        Provider::GitHub | Provider::GitHubEnterprise => github::list(account, auth)?,
        Provider::GitLab | Provider::GitLabSelfManaged => gitlab::list(account, auth)?,
        Provider::Bitbucket => bitbucket::list(auth)?,
        Provider::BitbucketDataCenter | Provider::AzureDevOps | Provider::JiraCloud | Provider::JiraDataCenter | Provider::Trello => {
            let name = account.provider.name();
            return Err(Error::Unsupported(format!("listing your repositories isn't supported on {name} yet")));
        }
    };
    let target = Target {
        provider: account.provider,
        host: account.host.clone(),
        owner: String::new(),
        project: String::new(),
        repo: String::new(),
    };
    let truncated = list.len() >= page::MAX_ITEMS;
    Ok(MyRepos { repos: list.into_iter().filter_map(|r| checked(&target, r)).collect(), truncated })
}

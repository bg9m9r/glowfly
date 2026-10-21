//! The size a hosting service reports for a repository, for the confirmation before fetching a
//! shallow clone's full history (`history_size`). GitHub reports KiB, GitLab its statistics
//! (only to Reporter access or more), Bitbucket Cloud and Azure DevOps bytes; the other
//! services report nothing.

use super::{Auth, Provider, Target, azure, bitbucket, get, github, gitlab};
use crate::error::Result;
use serde::Deserialize;

/// The repository's size in bytes as the service reports it; None when it reports none.
pub fn repo_size(t: &Target, auth: &Auth) -> Result<Option<u64>> {
    #[derive(Deserialize)]
    struct Sized {
        #[serde(default)]
        size: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Statistics {
        #[serde(default)]
        repository_size: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Project {
        #[serde(default)]
        statistics: Option<Statistics>,
    }
    match t.provider {
        Provider::GitHub | Provider::GitHubEnterprise => {
            Ok(get::<Sized>(auth, &github::repo_api(t))?.size.map(|kib| kib.saturating_mul(1024)))
        }
        Provider::GitLab | Provider::GitLabSelfManaged => {
            let p: Project = get(auth, &format!("{}?statistics=true", gitlab::project_api(t)))?;
            Ok(p.statistics.and_then(|s| s.repository_size))
        }
        Provider::Bitbucket => Ok(get::<Sized>(auth, &bitbucket::cloud_repo(t))?.size),
        Provider::AzureDevOps => Ok(get::<Sized>(auth, &format!("{}?api-version={}", azure::repo_api(t), azure::V))?.size),
        Provider::BitbucketDataCenter | Provider::JiraCloud | Provider::JiraDataCenter | Provider::Trello => Ok(None),
    }
}

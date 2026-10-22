//! How each service names a pull request's source when the branch is on a fork
//! (`NewPullRequest::source_repo`) rather than on the repository the pull request is opened on.
//! For a branch of the same repository every function gives what the service sent before.

use super::{Auth, NewPullRequest, Target, azure, get, gitlab};
use crate::error::Result;
use serde::Deserialize;
use serde_json::{Value, json};

/// GitHub's `head` and, when the fork has the same owner as `t`, its `head_repo` (the service
/// can't tell the repositories apart by owner alone then).
pub(super) fn github_head(t: &Target, pr: &NewPullRequest) -> (String, Option<String>) {
    match pr.fork_source(t) {
        None => (pr.source.clone(), None),
        Some(src) => {
            let head_repo = src.owner.eq_ignore_ascii_case(&t.owner).then(|| src.repo.clone());
            (format!("{}:{}", src.owner, pr.source), head_repo)
        }
    }
}

/// Where GitLab's merge request is posted (the source project's `merge_requests`) and, for a
/// fork, the target project's numeric id, read from the service.
pub(super) fn gitlab_post(t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<(String, Option<u64>)> {
    #[derive(Deserialize)]
    struct Project {
        id: u64,
    }
    match pr.fork_source(t) {
        None => Ok((format!("{}/merge_requests", gitlab::project_api(t)), None)),
        Some(src) => {
            let target: Project = get(auth, &gitlab::project_api(t))?;
            Ok((format!("{}/merge_requests", gitlab::project_api(src)), Some(target.id)))
        }
    }
}

/// Bitbucket Cloud's `source`: the branch, plus the fork's full name.
pub(super) fn bitbucket_source(pr: &NewPullRequest, t: &Target) -> Value {
    match pr.fork_source(t) {
        None => json!({ "branch": { "name": pr.source } }),
        Some(src) => json!({
            "branch": { "name": pr.source },
            "repository": { "full_name": format!("{}/{}", src.owner, src.repo) },
        }),
    }
}

/// Bitbucket Data Center's `fromRef.repository`: the fork's slug and project key (a personal
/// fork's `~user` key as-is), or `t`'s.
pub(super) fn dc_from_repo(pr: &NewPullRequest, t: &Target) -> Value {
    let repo = pr.fork_source(t).unwrap_or(t);
    json!({ "slug": repo.repo, "project": { "key": repo.owner } })
}

/// Azure DevOps' `forkSource`, by the fork repository's id read from the service; None for a
/// branch of the same repository.
pub(super) fn azure_fork_source(t: &Target, auth: &Auth, pr: &NewPullRequest) -> Result<Option<Value>> {
    let Some(src) = pr.fork_source(t) else { return Ok(None) };
    let info: azure::RepoInfo = get(auth, &format!("{}?api-version={}", azure::repo_api(src), azure::V))?;
    Ok(Some(json!({ "repository": { "id": info.id } })))
}

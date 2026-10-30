//! The repository a listed pull request's source branch is on (`PullRequest::head_repo`), read
//! from each service's list: the repository the pull request is opened on, a fork, or a fork
//! that was deleted. A local remote is the source's repository only when its URL parses to that
//! repository (https or SSH, letter case, a `.git` suffix and ports aside), so a same-named
//! branch on another repository is never taken for a fork's source.

use super::{Auth, Provider, Target, forks, get};
use serde::Deserialize;
use std::collections::HashMap;

/// At most this many GitLab fork projects are looked up per list; the rest stay known only by id.
const GITLAB_FORK_LOOKUPS: usize = 20;

/// A repository in a list answer, by full name: GitHub's `head.repo` and `base.repo`, Bitbucket
/// Cloud's `source.repository` and `destination.repository` (null for a deleted fork). A
/// missing name reads as empty: an unknown repository, not a failed list.
#[derive(Deserialize)]
pub(super) struct RepoName {
    #[serde(default)]
    full_name: String,
}

/// Where a pull request's source branch is, as the service's list says.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum HeadRepo {
    /// The repository the pull request is opened on (also when the list says nothing).
    #[default]
    Same,
    /// Another repository (a fork). `name` is its full name as the service gives it (`owner/repo`,
    /// a GitLab namespace path, a Data Center `PROJECT/repo`, an Azure `project/repo`); empty when
    /// it is unknown (a GitLab fork whose project couldn't be read, an entry without a name), and
    /// then it matches no remote.
    Fork { name: String },
    /// The fork was deleted: the list has no repository for the source.
    Deleted,
}

/// Azure DevOps' `forkSource`: set when the source branch is on a fork, its repository missing
/// once the fork is gone.
#[derive(Deserialize)]
pub(super) struct AzureFork {
    #[serde(default)]
    repository: Option<AzureRepo>,
}

#[derive(Deserialize)]
struct AzureRepo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    project: AzureProject,
}

#[derive(Deserialize, Default)]
struct AzureProject {
    #[serde(default)]
    name: String,
}

impl HeadRepo {
    /// Azure DevOps': the pull request's repository unless `forkSource` names another.
    pub(super) fn of_azure(fork: Option<&AzureFork>) -> HeadRepo {
        match fork.map(|f| f.repository.as_ref()) {
            None => HeadRepo::Same,
            Some(None) => HeadRepo::Deleted,
            Some(Some(r)) if r.name.is_empty() || r.project.name.is_empty() => HeadRepo::Fork { name: String::new() },
            Some(Some(r)) => HeadRepo::Fork { name: format!("{}/{}", r.project.name, r.name) },
        }
    }

    /// From the list's head and base repository names: the same one ignoring case, a fork, or a
    /// deleted fork when only the head's is missing. With neither (a list that doesn't say), the
    /// same repository.
    pub(super) fn of_names(head: Option<&str>, base: Option<&str>) -> HeadRepo {
        match (head, base) {
            (Some(""), _) => HeadRepo::Fork { name: String::new() },
            (Some(h), Some(b)) if h.eq_ignore_ascii_case(b) => HeadRepo::Same,
            (Some(h), _) => HeadRepo::Fork { name: h.to_owned() },
            (None, Some(_)) => HeadRepo::Deleted,
            (None, None) => HeadRepo::Same,
        }
    }

    /// From the list's source and target project ids (GitLab's): another project is a fork, named
    /// by its path from `paths` (`gitlab_fork_paths`) or else unknown; no source project is a
    /// deleted fork.
    pub(super) fn of_ids(head: Option<u64>, base: Option<u64>, paths: &HashMap<u64, String>) -> HeadRepo {
        match (head, base) {
            (Some(h), Some(b)) if h != b => HeadRepo::Fork { name: paths.get(&h).cloned().unwrap_or_default() },
            (None, Some(_)) => HeadRepo::Deleted,
            _ => HeadRepo::Same,
        }
    }

    /// `of_names` of a list's head and base repositories.
    pub(super) fn of_repos(head: &Option<RepoName>, base: &Option<RepoName>) -> HeadRepo {
        let name = |r: &Option<RepoName>| r.as_ref().map(|r| r.full_name.clone());
        HeadRepo::of_names(name(head).as_deref(), name(base).as_deref())
    }

    pub fn is_same(&self) -> bool {
        *self == HeadRepo::Same
    }

    /// Whether `repo` (a local remote's repository) is the one the source branch is on, for a
    /// pull request listed on `base`. A fork known only by id, and a deleted one, match nothing.
    pub fn is(&self, repo: &Target, base: &Target) -> bool {
        match self {
            HeadRepo::Same => forks::same_target(repo, base),
            HeadRepo::Fork { name } => fork_target(base, name).is_some_and(|f| forks::same_target(repo, &f)),
            HeadRepo::Deleted => false,
        }
    }
}

/// The paths (`path_with_namespace`) of the GitLab fork projects `ids`, each asked once, at most
/// `GITLAB_FORK_LOOKUPS`. One that can't be read (404, such as a private fork) is left out and its
/// fork stays unknown; any other error stops the lookups, as `checks::per_pr` does, leaving the
/// rest unknown too.
pub(super) fn gitlab_fork_paths(t: &Target, auth: &Auth, ids: impl IntoIterator<Item = u64>) -> HashMap<u64, String> {
    #[derive(Deserialize)]
    struct Project {
        path_with_namespace: String,
    }
    let mut ids: Vec<u64> = ids.into_iter().collect();
    ids.sort_unstable();
    ids.dedup();
    let mut out = HashMap::new();
    for id in ids.into_iter().take(GITLAB_FORK_LOOKUPS) {
        match get::<Project>(auth, &format!("https://{}/api/v4/projects/{id}", t.host)) {
            Ok(p) if !p.path_with_namespace.is_empty() => {
                out.insert(id, p.path_with_namespace);
            }
            Ok(_) => {}
            Err(e) if e.is_not_found() => {}
            Err(_) => break,
        }
    }
    out
}

/// The fork named `name` on `base`'s service: owner and repository split at the last `/` (an
/// Azure DevOps name is `project/repo` in `base`'s organization).
fn fork_target(base: &Target, name: &str) -> Option<Target> {
    let (owner, repo) = name.rsplit_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let t = if base.provider == Provider::AzureDevOps {
        Target { owner: base.owner.clone(), project: owner.to_owned(), repo: repo.to_owned(), ..base.clone() }
    } else {
        Target { owner: owner.to_owned(), project: String::new(), repo: repo.to_owned(), ..base.clone() }
    };
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::super::parse_remote;
    use super::*;

    fn remote(url: &str) -> Target {
        parse_remote(url, &[]).unwrap()
    }

    #[test]
    fn names_tell_same_fork_and_deleted_apart() {
        assert_eq!(HeadRepo::of_names(Some("Team/App"), Some("team/app")), HeadRepo::Same);
        assert_eq!(HeadRepo::of_names(Some("ada/app"), Some("team/app")), HeadRepo::Fork { name: "ada/app".into() });
        assert_eq!(HeadRepo::of_names(None, Some("team/app")), HeadRepo::Deleted);
        assert_eq!(HeadRepo::of_names(None, None), HeadRepo::Same);
        let paths = HashMap::from([(9, "bob/app".to_owned())]);
        assert_eq!(HeadRepo::of_ids(Some(7), Some(7), &paths), HeadRepo::Same);
        assert_eq!(HeadRepo::of_ids(Some(9), Some(7), &paths), HeadRepo::Fork { name: "bob/app".into() });
        assert_eq!(HeadRepo::of_ids(Some(8), Some(7), &paths), HeadRepo::Fork { name: String::new() }, "not read");
        assert_eq!(HeadRepo::of_ids(None, Some(7), &paths), HeadRepo::Deleted);
        assert_eq!(HeadRepo::of_ids(None, None, &paths), HeadRepo::Same);
        assert_eq!(HeadRepo::of_names(Some(""), Some("team/app")), HeadRepo::Fork { name: String::new() }, "a nameless head");
    }

    #[test]
    fn a_remote_is_the_fork_whatever_its_url_style() {
        let base = remote("https://github.com/team/app");
        let fork = HeadRepo::Fork { name: "ada/app".into() };
        for url in ["git@github.com:Ada/app.git", "https://github.com/ada/APP", "ssh://git@ssh.github.com:443/ada/app.git"] {
            assert!(fork.is(&remote(url), &base), "{url}");
        }
        assert!(!fork.is(&base, &base), "the base repository has a same-named branch, not the fork's");
        assert!(!fork.is(&remote("https://gitlab.com/ada/app"), &base), "another service");
        assert!(HeadRepo::Same.is(&remote("git@github.com:team/app.git"), &base));
        assert!(!HeadRepo::Same.is(&remote("https://github.com/ada/app"), &base));
        assert!(!HeadRepo::Deleted.is(&base, &base));
        assert!(!HeadRepo::Fork { name: String::new() }.is(&base, &base), "a fork known only by id");
    }

    #[test]
    fn ports_and_nested_groups_and_azure_projects() {
        let base = remote("https://gitlab.acme.io:8443/group/app");
        let fork = HeadRepo::Fork { name: "ada/sub/app".into() };
        assert!(fork.is(&remote("ssh://git@gitlab.acme.io:2222/ada/sub/app.git"), &base));
        let az = remote("https://dev.azure.com/contoso/Web/_git/app");
        let fork = HeadRepo::Fork { name: "Forks/app".into() };
        assert!(fork.is(&remote("git@ssh.dev.azure.com:v3/contoso/Forks/app"), &az));
        assert!(!fork.is(&az, &az));
    }
}

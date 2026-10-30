//! The common model every service maps to: pull requests with their reviewers, issues, and
//! a pull request to create.

use super::{Checks, HeadRepo, IssueFacets, PrFacets, Target};

#[derive(Clone, Debug, PartialEq)]
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
    /// The repository `source` is on (a fork's, or none left), from the service's list: a
    /// branch of that name on another repository isn't the source.
    pub head_repo: HeadRepo,
    pub target: String,
    pub author: String,
    pub draft: bool,
    pub reviewers: Vec<Reviewer>,
    /// Head commit SHA (Bitbucket Cloud reports only a 12-character prefix).
    pub head: Option<String>,
    pub merge_status: Option<String>,
    /// CI status of the head commit; None when the service reported none. On services asked once
    /// per pull request, the list carries it for the first `CHECKED_PRS` and `pr_details` fills
    /// in the rest.
    pub checks: Option<Checks>,
    /// Web page; empty when the service sent no usable link.
    pub url: String,
    /// What filters match on.
    pub facets: PrFacets,
}

impl PullRequest {
    pub fn approvals(&self) -> usize {
        self.reviewers.iter().filter(|r| r.state.starts_with("approved")).count()
    }

    pub fn head_matches(&self, sha: &str) -> bool {
        self.head.as_deref().is_some_and(|h| h.len() >= 7 && sha.starts_with(h))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    /// Display key: `123` for numbered trackers, `PROJ-123` for Jira.
    pub key: String,
    pub title: String,
    pub state: String,
    pub kind: String,
    pub assignee: Option<String>,
    pub url: String,
    pub facets: IssueFacets,
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
    /// The repository the source branch is on when it isn't the one the pull request is opened
    /// on (a fork); None for a branch of the same repository.
    pub source_repo: Option<Target>,
}

impl NewPullRequest {
    /// The fork the source branch is on when the pull request is opened on `t`; None when there
    /// is none or it names `t` itself (owner, project and repository compare ignoring case).
    pub fn fork_source(&self, t: &Target) -> Option<&Target> {
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        self.source_repo.as_ref().filter(|s| {
            !(s.provider == t.provider
                && same(&s.host, &t.host)
                && same(&s.owner, &t.owner)
                && same(&s.project, &t.project)
                && same(&s.repo, &t.repo))
        })
    }
}

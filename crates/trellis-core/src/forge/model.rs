//! The common model every service maps to: pull requests with their reviewers, issues, and
//! a pull request to create.

use super::{Checks, IssueFacets, PrFacets};

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
}

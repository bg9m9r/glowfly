//! What the forge effect asks a hosting service, and what comes back.

use glowfly_core::forge::{Auth, Checks, Issue, NewPullRequest, PrDetails, PullRequest, Target, TipQuery};
use std::collections::HashMap;

/// What to ask a hosting service.
#[derive(Debug)]
pub enum ForgeCall {
    /// Pull requests (plus the default branch and, when the host is also the tracker, issues).
    Refresh {
        need_default: bool,
        with_issues: bool,
    },
    /// Assigned issues, and (`project_keys`) the Jira site's project keys.
    MyIssues {
        project_keys: bool,
    },
    Issues(Vec<String>),
    Create(NewPullRequest),
    /// CI status of branch tips on the host.
    TipChecks(Vec<TipQuery>),
    /// The next chunk of the second pass: checks and votes of pull requests past the first
    /// `CHECKED_PRS` (up to `DETAILS_PER_CALL`).
    PrDetails(Vec<PullRequest>),
    Filter(super::FilterCall),
}

impl ForgeCall {
    /// Whether the app asks this on its own (refreshes, mentioned issues, checks, filters) rather
    /// than for something the user did. A background call for an account the profile hasn't
    /// connected uses only an environment token (`forge::background_auth`).
    pub fn is_background(&self) -> bool {
        !matches!(self, ForgeCall::Create(_))
    }
}

/// A hosting-service failure, flagged when signing in again could fix it.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgeError {
    pub message: String,
    pub auth: bool,
}

impl From<glowfly_core::error::Error> for ForgeError {
    fn from(e: glowfly_core::error::Error) -> Self {
        ForgeError { auth: e.is_auth(), message: e.to_string() }
    }
}

#[derive(Debug)]
pub enum ForgeResult {
    AuthFailed(String),
    Prs(Result<Vec<PullRequest>, ForgeError>),
    MyIssues(Result<Vec<Issue>, ForgeError>),
    Issues(Vec<String>, Result<Vec<Issue>, ForgeError>),
    DefaultBranch(Option<String>),
    /// Boxed: a pull request is far larger than the other results.
    Created(Result<Box<PullRequest>, ForgeError>),
    /// The tips asked about, and their status by full commit id.
    TipChecks(Vec<TipQuery>, Result<HashMap<String, Checks>, ForgeError>),
    PrDetails(Result<Vec<PrDetails>, ForgeError>),
    /// A Jira site's project keys; None when it has too many to list.
    ProjectKeys(Result<Option<Vec<String>>, ForgeError>),
    Filter(super::FilterResult),
}

#[derive(Debug)]
pub struct ForgeReply {
    pub target: Target,
    pub auth: Option<Auth>,
    pub results: Vec<ForgeResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_creating_a_pull_request_is_the_users_own_call() {
        let new = NewPullRequest {
            source: "topic".into(),
            target: "main".into(),
            title: "t".into(),
            description: String::new(),
            draft: false,
            issues: vec![],
            source_repo: None,
        };
        assert!(!ForgeCall::Create(new).is_background());
        for call in [
            ForgeCall::Refresh { need_default: true, with_issues: true },
            ForgeCall::MyIssues { project_keys: false },
            ForgeCall::Issues(vec![]),
            ForgeCall::TipChecks(vec![]),
            ForgeCall::PrDetails(vec![]),
        ] {
            assert!(call.is_background(), "{call:?}");
        }
    }
}

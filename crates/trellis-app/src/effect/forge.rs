//! What the forge effect asks a hosting service, and what comes back.

use trellis_core::forge::{Auth, Issue, NewPullRequest, PullRequest, Target};

/// What to ask a hosting service.
#[derive(Debug)]
pub enum ForgeCall {
    /// Pull requests (plus the default branch and, when the host is also the tracker, issues).
    Refresh {
        need_default: bool,
        with_issues: bool,
    },
    MyIssues,
    Issues(Vec<String>),
    Create(NewPullRequest),
}

/// A hosting-service failure, flagged when signing in again could fix it.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgeError {
    pub message: String,
    pub auth: bool,
}

impl From<trellis_core::error::Error> for ForgeError {
    fn from(e: trellis_core::error::Error) -> Self {
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
    Created(Result<PullRequest, ForgeError>),
}

#[derive(Debug)]
pub struct ForgeReply {
    pub target: Target,
    pub auth: Option<Auth>,
    pub results: Vec<ForgeResult>,
}

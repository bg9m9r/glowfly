//! What the cross-repository dashboard asks the runner (runner/dashboard.rs): reading the known
//! repositories' remotes with libgit2, then one round of service calls, and what comes back.

use super::EffectMeta;
use super::ForgeError;
#[cfg(test)]
use super::SandboxRule;
use glowfly_core::forge::{Account, Auth, Connection, Issue, Me, PullRequest, Target};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum DashboardEffect {
    /// Opens each folder with libgit2 and reads its recognized remotes.
    Scan { generation: u64, repos: Vec<PathBuf>, connections: Vec<Connection> },
    /// Signs in to each account and asks its targets.
    Fetch(Box<FetchJob>),
}

#[derive(Debug)]
pub struct FetchJob {
    pub generation: u64,
    /// Where credential helpers run: outside every repository, so none of their configs apply.
    pub cwd: PathBuf,
    /// Git Credential Manager may show its sign-in window (Sign in for one account only).
    pub interactive: bool,
    /// Only some accounts are asked: the reply replaces just their targets.
    pub partial: bool,
    pub accounts: Vec<AccountJob>,
}

#[derive(Debug)]
pub struct AccountJob {
    pub account: Account,
    pub connected: bool,
    /// The credential cached from an earlier reply; None resolves it again.
    pub auth: Option<Auth>,
    /// The signed-in user, when known already.
    pub me: Option<Me>,
    pub targets: Vec<TargetJob>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetJob {
    pub target: Target,
    pub prs: bool,
    pub issues: bool,
}

#[derive(Debug)]
pub struct FetchReply {
    pub generation: u64,
    pub partial: bool,
    /// When the calls finished (Unix seconds).
    pub fetched_at: i64,
    pub accounts: Vec<AccountReply>,
}

#[derive(Debug)]
pub struct AccountReply {
    pub account: Account,
    pub auth: Result<Auth, String>,
    /// The signed-in user, when it was looked up.
    pub me: Option<Result<Me, ForgeError>>,
    /// Empty when signing in failed.
    pub targets: Vec<TargetReply>,
}

#[derive(Debug)]
pub struct TargetReply {
    pub target: Target,
    pub prs: Option<Result<Vec<PullRequest>, ForgeError>>,
    pub issues: Option<Result<Vec<Issue>, ForgeError>>,
}

impl EffectMeta for DashboardEffect {
    fn name(&self) -> &'static str {
        match self {
            DashboardEffect::Scan { .. } => "dashboard-scan",
            DashboardEffect::Fetch(_) => "dashboard-fetch",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Timed for the performance log, except a sign-in that may wait on the user.
    fn timed(&self) -> bool {
        match self {
            DashboardEffect::Scan { .. } => true,
            DashboardEffect::Fetch(job) => !job.interactive,
        }
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}

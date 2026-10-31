//! The cross-repository dashboard: pull requests and issues of every repository Glowfly knows
//! (open tabs, favorites, then recent), grouped by service account and then by repository. Its
//! state is global, never parked with a tab, and kept while the dashboard is closed. Which
//! repositories and calls a refresh plans is in `dashboard_plan.rs`; the rows the view shows are
//! in `dashboard_rows.rs`.

use super::AppState;
use super::FORGE_REFRESH_INTERVAL;
use glowfly_core::forge::{Account, Auth, Connection, Issue, Me, Provider, PullRequest, Target};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// The most folders read per refresh.
pub const MAX_REPOS: usize = 200;
/// The most repositories (or tracker sites and boards) asked per refresh.
pub const MAX_TARGETS: usize = 50;

/// The three-way switch over the rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DashShow {
    /// Open pull requests the signed-in user opened.
    #[default]
    Mine,
    /// Open pull requests waiting for the user's review.
    Review,
    /// Open pull requests, issues, work items and cards assigned to the user.
    Assigned,
}

impl DashShow {
    pub const ALL: [DashShow; 3] = [DashShow::Mine, DashShow::Review, DashShow::Assigned];

    pub fn label(self) -> &'static str {
        match self {
            DashShow::Mine => "Mine",
            DashShow::Review => "Review requested",
            DashShow::Assigned => "Assigned",
        }
    }

    pub fn index(self) -> usize {
        match self {
            DashShow::Mine => 0,
            DashShow::Review => 1,
            DashShow::Assigned => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Reading the repositories' remotes.
    Scanning,
    /// Asking the services.
    Fetching,
}

/// A local clone of a target's repository (or a repository whose issues live on it).
#[derive(Clone, Debug, PartialEq)]
pub struct LocalRepo {
    /// As Glowfly lists it (a tab, a favorite or a recent entry).
    pub path: PathBuf,
    /// Where it really is (`Snapshot::real_workdir`).
    pub real: PathBuf,
    pub name: String,
    /// The remote that names the target; empty for an issue tracker chosen in settings.
    pub remote: String,
    pub remote_url: String,
    /// The target is this clone's code host, as its tab would pick it.
    pub is_host: bool,
}

/// One repository, Jira site or Trello board, and what it answered.
#[derive(Clone, Debug)]
pub struct TargetData {
    pub target: Target,
    pub locals: Vec<LocalRepo>,
    pub wants_prs: bool,
    pub wants_issues: bool,
    /// None until asked (or when not wanted).
    pub prs: Option<Result<Vec<PullRequest>, String>>,
    pub issues: Option<Result<Vec<Issue>, String>>,
}

impl TargetData {
    pub fn new(target: Target) -> Self {
        Self { target, locals: Vec::new(), wants_prs: false, wants_issues: false, prs: None, issues: None }
    }

    /// The local folder names, for the repository row.
    pub fn folder_names(&self) -> Vec<&str> {
        self.locals.iter().map(|l| l.name.as_str()).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccountState {
    /// Being asked.
    Pending,
    Ok,
    /// A self-hosted server only guessed from a remote URL: never contacted until connected.
    NotConnected,
    SignInFailed(String),
}

/// A service account the targets belong to.
#[derive(Clone, Debug)]
pub struct AccountRow {
    pub account: Account,
    pub state: AccountState,
    pub me: Option<Me>,
    /// Looking the user up failed: pull requests can't be split into Mine and Review.
    pub me_error: Option<String>,
}

impl AccountRow {
    pub fn new(account: Account, state: AccountState) -> Self {
        Self { account, state, me: None, me_error: None }
    }
}

/// What a refresh was planned from: when either changes, the results are stale.
#[derive(Clone, Debug, PartialEq)]
pub struct Basis {
    pub repos: Vec<PathBuf>,
    pub connections: Vec<Connection>,
}

/// "Show pull request" for a repository whose tab hasn't listed its pull requests yet.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPr {
    pub path: PathBuf,
    pub target: Target,
    pub id: u64,
}

#[derive(Default)]
pub struct Dashboard {
    pub open: bool,
    pub show: DashShow,
    /// The filter field's text buffer.
    pub filter: String,
    pub generation: u64,
    pub phase: Phase,
    /// Fetch replies still to come for this generation.
    pub in_flight: usize,
    /// When the last full refresh finished (Unix seconds), for "Updated … ago".
    pub fetched_at: Option<i64>,
    /// The same moment, for the refresh interval.
    pub fetched: Option<Instant>,
    pub basis: Option<Basis>,
    pub targets: Vec<TargetData>,
    pub accounts: Vec<AccountRow>,
    /// Readable repositories without a hosting-service remote or tracker.
    pub no_service: Vec<PathBuf>,
    /// Folders that couldn't be read, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Targets past `MAX_TARGETS`, not asked.
    pub over_limit: usize,
    /// Credentials by account key.
    pub auth: HashMap<String, Auth>,
    pub pending_pr: Option<PendingPr>,
}

impl Dashboard {
    /// Whether the results need asking again: never fetched, older than the refresh interval, or
    /// planned from other repositories or connections.
    pub fn is_stale(&self, basis: &Basis, now: Instant) -> bool {
        let old = self.fetched.is_none_or(|t| now.duration_since(t) >= FORGE_REFRESH_INTERVAL);
        old || self.basis.as_ref() != Some(basis)
    }

    pub fn account(&self, account: &Account) -> Option<&AccountRow> {
        self.accounts.iter().find(|a| a.account == *account)
    }

    pub fn account_mut(&mut self, account: &Account) -> Option<&mut AccountRow> {
        self.accounts.iter_mut().find(|a| a.account == *account)
    }

    pub fn is_busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// The clone to open for `data`: one open in a tab, then a favorite, then the first listed.
    pub fn preferred_local<'a>(data: &'a TargetData, st: &AppState) -> Option<&'a LocalRepo> {
        let open = data.locals.iter().find(|l| st.tab_showing(&l.path).is_some());
        let fav = || data.locals.iter().find(|l| super::repo_lists::is_favorite(st, &l.path));
        open.or_else(fav).or(data.locals.first())
    }

    /// The clone whose code host is `data`'s target, where "Show pull request" opens; else why
    /// there is none.
    pub fn pr_host_local(data: &TargetData) -> Result<&LocalRepo, String> {
        data.locals
            .iter()
            .find(|l| l.is_host)
            .ok_or_else(|| format!("No clone here uses {} as its code host; open it in the browser instead", target_name(&data.target)))
    }
}

/// How a target is named on its row: "acme/widgets", "acme.atlassian.net · PROJ", "Trello · Board".
pub fn target_name(t: &Target) -> String {
    if t.provider == Provider::Trello { format!("Trello · {}", t.display()) } else { t.display() }
}

/// "GitHub · github.com", and who is signed in once known.
pub fn service_header(row: &AccountRow) -> String {
    let mut s = format!("{} · {}", row.account.provider.name(), row.account.display());
    if let Some(me) = &row.me {
        let who = if me.name.trim().is_empty() { &me.id } else { &me.name };
        s.push_str(&format!(" · signed in as {who}"));
    }
    s
}

/// "#12" or "!7": how a pull request on `t` is labelled.
pub fn pr_label(t: &Target, id: u64) -> String {
    format!("{}{id}", t.provider.pr_prefix())
}

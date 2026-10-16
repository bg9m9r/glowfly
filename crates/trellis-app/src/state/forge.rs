//! Pull requests and issues for the open repository.

use super::build_status::TipChecks;
use git2::Oid;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use trellis_core::forge::{self, Auth, Issue, Provider, PullRequest, RefStyle, Target};

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Idle,
    Ok,
    Error(String),
}

/// The code host comes from the remote URL, the issue tracker from Repo-Specific Preferences
/// (the host's own issues by default).
pub struct ForgeState {
    pub host: Option<Target>,
    /// Git remote that points at `host`.
    pub remote: String,
    pub tracker: Option<Target>,
    /// Credentials by account key.
    pub auth: HashMap<String, Auth>,
    pub host_status: Status,
    pub tracker_status: Status,
    /// Requests in flight.
    pub loading: usize,
    pub prs: Vec<PullRequest>,
    /// Graph commit for each PR's head.
    pub pr_heads: HashMap<u64, Oid>,
    pub my_issues: Vec<Issue>,
    pub cache: HashMap<String, Issue>,
    pub pending: HashSet<String>,
    pub default_branch: Option<String>,
    pub last_refresh: Option<Instant>,
    /// CI status of the branch tips on screen.
    pub tips: TipChecks,
    /// A push for a pull request succeeded: open its create dialog when the next snapshot lands.
    pub open_after_load: Option<super::push_pr::PrStart>,
}

impl ForgeState {
    pub fn new(host: Option<Target>, remote: String, tracker: Option<Target>) -> Self {
        Self {
            host,
            remote,
            tracker,
            auth: HashMap::new(),
            host_status: Status::Idle,
            tracker_status: Status::Idle,
            loading: 0,
            prs: Vec::new(),
            pr_heads: HashMap::new(),
            my_issues: Vec::new(),
            cache: HashMap::new(),
            pending: HashSet::new(),
            default_branch: None,
            last_refresh: None,
            tips: TipChecks::default(),
            open_after_load: None,
        }
    }

    /// Replies in flight were dropped (the tab was parked): nothing is loading any more.
    pub fn replies_dropped(&mut self) {
        self.loading = 0;
        self.tips.in_flight.clear();
    }

    pub fn pr_for_branch(&self, branch: &str) -> Option<&PullRequest> {
        self.prs.iter().find(|p| p.source == branch)
    }

    pub fn host_provider(&self) -> Option<Provider> {
        self.host.as_ref().map(|t| t.provider)
    }

    pub fn pr_noun(&self) -> &'static str {
        self.host_provider().map(Provider::pr_noun).unwrap_or("pull request")
    }

    pub fn pr_label(&self, id: u64) -> String {
        format!("{}{id}", self.host_provider().map(Provider::pr_prefix).unwrap_or("#"))
    }

    pub fn style(&self) -> Option<RefStyle> {
        self.tracker.as_ref().map(|t| t.provider.ref_style())
    }

    pub fn issue_refs(&self, text: &str) -> Vec<String> {
        let mut refs = self.style().map(|s| forge::issue_refs(s, text)).unwrap_or_default();
        // With a Jira project configured, only that project's keys count.
        if let Some(t) = self.tracker.as_ref().filter(|t| t.provider.is_jira() && !t.project.is_empty()) {
            let prefix = format!("{}-", t.project);
            refs.retain(|k| k.starts_with(&prefix));
        }
        refs
    }

    pub fn branch_issue(&self, branch: &str) -> Option<String> {
        self.style().and_then(|s| forge::branch_issue(s, branch))
    }

    pub fn mention(&self, key: &str) -> String {
        forge::mention(self.style().unwrap_or(RefStyle::Hash), key)
    }

    pub fn issue_url(&self, key: &str) -> String {
        self.cache
            .get(key)
            .map(|i| i.url.clone())
            .filter(|u| !u.is_empty())
            .or_else(|| self.tracker.as_ref().map(|t| t.issue_url(key)))
            .unwrap_or_default()
    }

    pub fn issue_noun(&self) -> &'static str {
        match self.tracker.as_ref().map(|t| t.provider) {
            Some(Provider::AzureDevOps) => "work item",
            Some(Provider::Trello) => "card",
            _ => "issue",
        }
    }

    fn is_trello(&self) -> bool {
        self.tracker.as_ref().is_some_and(|t| t.provider == Provider::Trello)
    }

    /// The ISSUES row menu item that opens an issue's web page.
    pub fn open_issue_label(&self) -> &'static str {
        if self.is_trello() { "View card in Trello" } else { "Open in browser" }
    }

    /// The ISSUES row menu item that copies an issue's reference, and what it copies: a card's
    /// link (Trello has no mention syntax), else the mention.
    pub fn copy_issue_item(&self, issue: &Issue) -> (String, String) {
        if self.is_trello() {
            return ("Copy card link".into(), issue.url.clone());
        }
        let m = self.mention(&issue.key);
        (format!("Copy {m}"), m)
    }

    /// The tracker over the ISSUES list: its name, and for Trello the board.
    pub fn tracker_hint(&self) -> String {
        match self.tracker.as_ref().filter(|_| self.is_trello()) {
            Some(t) => format!("Trello · {}", t.display()),
            None => self.tracker_name().to_owned(),
        }
    }

    pub fn tracker_name(&self) -> &'static str {
        match self.tracker.as_ref().map(|t| t.provider) {
            Some(Provider::AzureDevOps) => "Azure Boards",
            Some(p) => p.name(),
            None => "",
        }
    }
}

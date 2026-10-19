//! Pull requests and issues for the open repository.

use super::build_status::TipChecks;
use super::pr_details::PrDetailPass;
use git2::Oid;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use trellis_core::forge::{self, Account, Auth, Issue, Provider, PullRequest, RefStyle, Target};
use trellis_core::i18n::Lang;
use trellis_core::settings::TrackerChoice;

/// A Jira site's project keys, read once per tracker to tell keys from look-alikes (`UTF-8`).
#[derive(Clone, PartialEq, Debug, Default)]
pub enum ProjectKeys {
    /// Not read yet, or the read failed (asked again on the next refresh).
    #[default]
    Unknown,
    /// More projects than a list holds: the built-in word list applies.
    TooMany,
    Known(HashSet<String>),
}

/// The issue tracker a repository with code host `host` uses under `choice` (Auto: the host's
/// own issues, when it has them).
pub fn tracker_target(choice: TrackerChoice, host: Option<&Target>) -> Option<Target> {
    match choice {
        TrackerChoice::Auto => host.filter(|t| t.provider.has_issues()).cloned(),
        TrackerChoice::None => None,
        TrackerChoice::Jira { account, project } => Some(Target::jira(&account, &project)),
        TrackerChoice::Trello { account, board, board_name } => Some(Target::trello(&account, &board, &board_name)),
    }
}

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
    /// Checks and votes past the first `CHECKED_PRS`, on services read per pull request.
    pub details: PrDetailPass,
    /// The Jira tracker's project keys.
    pub jira_projects: ProjectKeys,
    /// The signed-in user, issue searches and dialog options of PR and issue filters.
    pub filters: super::forge_filters::FilterState,
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
            details: PrDetailPass::default(),
            jira_projects: ProjectKeys::Unknown,
            filters: Default::default(),
        }
    }

    /// Replies in flight were dropped (the tab was parked): nothing is loading any more.
    pub fn replies_dropped(&mut self) {
        self.loading = 0;
        self.tips.in_flight.clear();
        self.details.in_flight = false;
        self.details.asking.clear();
        self.details.queue.clear();
        self.filters.replies_dropped();
    }

    pub fn pr_for_branch(&self, branch: &str) -> Option<&PullRequest> {
        self.prs.iter().find(|p| p.source == branch)
    }

    /// Whether the code host and the tracker are on `account`.
    pub fn uses_account(&self, account: &Account) -> (bool, bool) {
        let on = |t: &Option<Target>| t.as_ref().is_some_and(|t| t.account() == *account);
        (on(&self.host), on(&self.tracker))
    }

    pub fn host_provider(&self) -> Option<Provider> {
        self.host.as_ref().map(|t| t.provider)
    }

    pub fn pr_noun(&self) -> &'static str {
        self.host_provider().map(Provider::pr_noun).unwrap_or("pull request")
    }

    /// What this host calls a pull request, in `t` ("merge request" on GitLab).
    pub fn pr_word(&self, t: Lang) -> &'static str {
        if self.pr_noun() == "merge request" { t.tr("forge.noun.merge_request") } else { t.tr("forge.noun.pull_request") }
    }

    /// The PULL REQUESTS section's header, in `t` ("Merge requests" on GitLab).
    pub fn prs_header(&self, t: Lang) -> &'static str {
        if self.pr_noun() == "merge request" { t.tr("sidebar.mrs.header") } else { t.tr("sidebar.prs.header") }
    }

    pub fn pr_label(&self, id: u64) -> String {
        format!("{}{id}", self.host_provider().map(Provider::pr_prefix).unwrap_or("#"))
    }

    pub fn style(&self) -> Option<RefStyle> {
        self.tracker.as_ref().map(|t| t.provider.ref_style())
    }

    pub fn issue_refs(&self, text: &str) -> Vec<String> {
        let mut refs = self.style().map(|s| forge::issue_refs(s, text)).unwrap_or_default();
        refs.retain(|k| self.keeps(k));
        refs
    }

    pub fn branch_issue(&self, branch: &str) -> Option<String> {
        self.style().and_then(|s| forge::branch_issue(s, branch)).filter(|k| self.keeps(k))
    }

    /// Jira keys count only for the project filter, else the site's projects once known.
    fn keeps(&self, key: &str) -> bool {
        match self.tracker.as_ref().filter(|t| t.provider.is_jira()) {
            Some(t) => forge::keep_jira_key(key, &t.project, self.known_projects()),
            None => true,
        }
    }

    pub fn known_projects(&self) -> Option<&HashSet<String>> {
        match &self.jira_projects {
            ProjectKeys::Known(set) => Some(set),
            _ => None,
        }
    }

    /// A Jira tracker without a project filter whose project keys haven't been read.
    pub fn needs_project_keys(&self) -> bool {
        self.tracker.as_ref().is_some_and(|t| t.provider.is_jira() && t.project.trim().is_empty())
            && self.jira_projects == ProjectKeys::Unknown
    }

    pub fn mention(&self, key: &str) -> String {
        forge::mention(self.style().unwrap_or(RefStyle::Hash), key)
    }

    /// The text a commit message carries to link `key`: the mention, or for Trello (which has no
    /// mention syntax) the card's link.
    pub fn message_ref(&self, key: &str) -> String {
        if self.is_trello() { self.issue_url(key) } else { self.mention(key) }
    }

    /// Whether `text` already links `key` (see [`Self::message_ref`]).
    pub fn links_issue(&self, text: &str, key: &str) -> bool {
        if self.is_trello() { self.issue_refs(text).iter().any(|k| k == key) } else { text.contains(&self.mention(key)) }
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
        self.tracker.as_ref().map_or("issue", |t| super::issue_words::noun(t.provider))
    }

    /// What this tracker calls an issue, in `t` ("work item", "card").
    pub fn issue_word(&self, t: Lang) -> &'static str {
        match self.tracker.as_ref().map(|t| t.provider) {
            Some(Provider::AzureDevOps) => t.tr("forge.noun.work_item"),
            Some(Provider::Trello) => t.tr("forge.noun.card"),
            _ => t.tr("forge.noun.issue"),
        }
    }

    /// The commit details' heading over the issue chips, in this tracker's word.
    pub fn issues_header(&self, t: Lang) -> &'static str {
        match self.tracker.as_ref().map(|t| t.provider) {
            Some(Provider::AzureDevOps) => t.tr("details.issues.work_item.header"),
            Some(Provider::Trello) => t.tr("details.issues.card.header"),
            _ => t.tr("details.issues.issue.header"),
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

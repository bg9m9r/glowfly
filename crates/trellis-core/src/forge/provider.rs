//! What differs between hosting services, in one place: every [`Provider`] has a
//! [`ProviderInfo`] with its names, sign-in hints and conventions, and the [`Forge`] that talks
//! to its API. Adding a service means a `Provider` variant, its `ProviderInfo` (next to its API
//! code) and an arm in [`Provider::info`].

use super::{Forge, RefStyle, azure, bitbucket, bitbucket_dc, github, gitlab, jira, trello};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    GitHub,
    GitHubEnterprise,
    GitLab,
    GitLabSelfManaged,
    Bitbucket,
    BitbucketDataCenter,
    AzureDevOps,
    JiraCloud,
    JiraDataCenter,
    Trello,
}

/// One service's constants and API implementation.
pub struct ProviderInfo {
    pub name: &'static str,
    /// Public SaaS host, for services that have exactly one.
    pub fixed_host: Option<&'static str>,
    /// Label for an extra username field, when the token must be paired with one.
    pub username_label: Option<&'static str>,
    /// Placeholder for the host field in Preferences.
    pub host_hint: &'static str,
    /// What kind of token to create, for Preferences.
    pub token_help: &'static str,
    /// Environment variable checked before saved credentials (on the public host only).
    pub env_var: Option<&'static str>,
    /// "pull request", or GitLab's "merge request".
    pub pr_noun: &'static str,
    /// Written before a pull request's number: `#` or `!`.
    pub pr_prefix: &'static str,
    /// How issues are mentioned in text.
    pub ref_style: RefStyle,
    pub has_issues: bool,
    /// Jira Cloud or Jira Data Center.
    pub is_jira: bool,
    /// An issue tracker only, without repositories, pull requests or Git Credential Manager sign-in.
    pub tracker_only: bool,
    /// The word in a pull request description that closes an issue on merge (`Closes #3`).
    pub close_keyword: Option<&'static str>,
    /// Links its own issues to pull requests through the API rather than by mention.
    pub links_issues_by_api: bool,
    /// Can list a repository's forks (the Add a fork dialog).
    pub lists_forks: bool,
    /// Can fork a repository into the signed-in user's account.
    pub creates_forks: bool,
    /// Can add an SSH public key to the signed-in user's account.
    pub adds_ssh_keys: bool,
    /// Reads checks or votes with one call per pull request, so the list carries them only for
    /// the first `CHECKED_PRS` (the rest come from `pr_details`).
    pub details_per_pr: bool,
    /// Issue details and creating issues work inside Trellis.
    pub issues_in_app: bool,
    /// Request headers only this service understands.
    pub api_headers: &'static [(&'static str, &'static str)],
    /// The API implementation.
    pub service: &'static dyn Forge,
}

impl Provider {
    /// In the order Preferences lists them.
    pub const ALL: [Provider; 10] = [
        Provider::GitHub,
        Provider::GitHubEnterprise,
        Provider::GitLab,
        Provider::GitLabSelfManaged,
        Provider::Bitbucket,
        Provider::BitbucketDataCenter,
        Provider::AzureDevOps,
        Provider::JiraCloud,
        Provider::JiraDataCenter,
        Provider::Trello,
    ];

    pub fn info(self) -> &'static ProviderInfo {
        match self {
            Provider::GitHub => &github::GITHUB,
            Provider::GitHubEnterprise => &github::GITHUB_ENTERPRISE,
            Provider::GitLab => &gitlab::GITLAB,
            Provider::GitLabSelfManaged => &gitlab::GITLAB_SELF_MANAGED,
            Provider::Bitbucket => &bitbucket::BITBUCKET,
            Provider::BitbucketDataCenter => &bitbucket_dc::BITBUCKET_DATA_CENTER,
            Provider::AzureDevOps => &azure::AZURE_DEVOPS,
            Provider::JiraCloud => &jira::JIRA_CLOUD,
            Provider::JiraDataCenter => &jira::JIRA_DATA_CENTER,
            Provider::Trello => &trello::TRELLO,
        }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn is_jira(self) -> bool {
        self.info().is_jira
    }

    pub fn is_tracker_only(self) -> bool {
        self.info().tracker_only
    }

    pub fn fixed_host(self) -> Option<&'static str> {
        self.info().fixed_host
    }

    /// Git Credential Manager can sign in to every code host (not the issue trackers).
    pub fn supports_gcm(self) -> bool {
        !self.is_tracker_only()
    }

    pub fn username_label(self) -> Option<&'static str> {
        self.info().username_label
    }

    pub fn host_hint(self) -> &'static str {
        self.info().host_hint
    }

    pub fn token_help(self) -> &'static str {
        self.info().token_help
    }

    pub fn env_var(self) -> Option<&'static str> {
        self.info().env_var
    }

    pub fn pr_noun(self) -> &'static str {
        self.info().pr_noun
    }

    pub fn pr_prefix(self) -> &'static str {
        self.info().pr_prefix
    }

    pub fn ref_style(self) -> RefStyle {
        self.info().ref_style
    }

    pub fn has_issues(self) -> bool {
        self.info().has_issues
    }

    pub fn lists_forks(self) -> bool {
        self.info().lists_forks
    }

    pub fn creates_forks(self) -> bool {
        self.info().creates_forks
    }

    pub fn adds_ssh_keys(self) -> bool {
        self.info().adds_ssh_keys
    }

    pub fn details_per_pr(self) -> bool {
        self.info().details_per_pr
    }

    pub fn issues_in_app(self) -> bool {
        self.info().issues_in_app
    }
}

/// The settings most services share; each `ProviderInfo` overrides what differs.
pub(super) const DEFAULTS: ProviderInfo = ProviderInfo {
    name: "",
    fixed_host: None,
    username_label: None,
    host_hint: "",
    token_help: "",
    env_var: None,
    pr_noun: "pull request",
    pr_prefix: "#",
    ref_style: RefStyle::Hash,
    has_issues: true,
    is_jira: false,
    tracker_only: false,
    close_keyword: None,
    links_issues_by_api: false,
    lists_forks: false,
    creates_forks: false,
    adds_ssh_keys: false,
    details_per_pr: false,
    issues_in_app: false,
    api_headers: &[],
    service: &NoService,
};

/// Stands in for `DEFAULTS.service`; every real `ProviderInfo` names its own.
struct NoService;

impl Forge for NoService {
    fn verify(&self, _: &super::Account, _: &super::Auth) -> crate::error::Result<String> {
        Err(crate::error::Error::Unsupported("no service".into()))
    }
    fn token_url(&self, _: &super::Account) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_its_own_info() {
        for p in Provider::ALL {
            let info = p.info();
            assert!(!info.name.is_empty() && !info.token_help.is_empty(), "{p:?}");
            assert_eq!(info.fixed_host.is_none(), !info.host_hint.is_empty(), "{p:?}: a host to type, or a fixed one");
            // Each names its own service rather than the placeholder (which has no token page).
            let account = super::super::Account { provider: p, host: "h.example".into(), org: "o".into() };
            assert!(info.service.token_url(&account).is_some(), "{p:?}");
        }
        let names: std::collections::HashSet<_> = Provider::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(names.len(), Provider::ALL.len());
    }

    #[test]
    fn conventions_per_provider() {
        use Provider::*;
        let row = |p: Provider| (p.pr_prefix(), p.pr_noun(), p.env_var(), p.username_label(), p.ref_style(), p.has_issues());
        let hash = RefStyle::Hash;
        assert_eq!(row(GitHub), ("#", "pull request", Some("GITHUB_TOKEN"), None, hash, true));
        assert_eq!(row(GitHubEnterprise), ("#", "pull request", Some("GITHUB_TOKEN"), None, hash, true));
        assert_eq!(row(GitLab), ("!", "merge request", Some("GITLAB_TOKEN"), None, hash, true));
        assert_eq!(row(GitLabSelfManaged), ("!", "merge request", Some("GITLAB_TOKEN"), None, hash, true));
        assert_eq!(row(Bitbucket), ("#", "pull request", None, Some("Username"), hash, true));
        assert_eq!(row(BitbucketDataCenter), ("#", "pull request", None, None, hash, false));
        assert_eq!(row(AzureDevOps), ("!", "pull request", Some("AZURE_DEVOPS_EXT_PAT"), None, RefStyle::AzureBoards, true));
        assert_eq!(row(JiraCloud), ("#", "pull request", None, Some("Email"), RefStyle::Jira, true));
        assert_eq!(row(JiraDataCenter), ("#", "pull request", None, None, RefStyle::Jira, true));
        assert_eq!(row(Trello), ("#", "pull request", None, Some("API Key"), RefStyle::Trello, true));
        let fixed: Vec<_> = Provider::ALL.iter().filter_map(|p| p.fixed_host()).collect();
        assert_eq!(fixed, ["github.com", "gitlab.com", "bitbucket.org", "trello.com"]);
        assert!(Provider::ALL.iter().all(|p| p.supports_gcm() != p.is_tracker_only()));
        assert_eq!(Provider::ALL.iter().filter(|p| p.is_jira()).count(), 2);
        assert_eq!(Provider::ALL.iter().filter(|p| p.is_tracker_only()).count(), 3);
        let forks = |p: Provider| (p.lists_forks(), p.creates_forks());
        for p in [GitHub, GitHubEnterprise, GitLab, GitLabSelfManaged] {
            assert_eq!(forks(p), (true, true), "{p:?}");
        }
        for p in [Bitbucket, BitbucketDataCenter] {
            assert_eq!(forks(p), (true, false), "{p:?}");
        }
        for p in [AzureDevOps, JiraCloud, JiraDataCenter, Trello] {
            assert_eq!(forks(p), (false, false), "{p:?}");
        }
        for p in Provider::ALL {
            let adds = matches!(p, GitHub | GitHubEnterprise | GitLab | GitLabSelfManaged | Bitbucket);
            assert_eq!(p.adds_ssh_keys(), adds, "{p:?}");
            let per_pr = matches!(p, GitLab | GitLabSelfManaged | Bitbucket | BitbucketDataCenter | AzureDevOps);
            assert_eq!(p.details_per_pr(), per_pr, "{p:?}");
            assert_eq!(p.issues_in_app(), p != BitbucketDataCenter, "{p:?}");
        }
    }
}

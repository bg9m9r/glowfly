//! The Clone dialog's "Your repositories" tab: the connected accounts that can list their
//! repositories, the list loaded for the chosen one, the filter, the pick and which clone URL
//! (https or SSH) the pick fills in.

use crate::effect::ForgeError;
use trellis_core::forge::{self, Account, Connection, HostedRepo, MyRepos, Provider};

/// Where the Clone dialog's URL comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloneSource {
    /// Typed or pasted.
    #[default]
    Url,
    /// Picked from the user's repositories on a connected service.
    Service,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServicePicker {
    /// The accounts the tab offers (`listing_accounts`).
    pub accounts: Vec<Account>,
    /// The chosen one, an index into `accounts`.
    pub account: usize,
    pub loading: bool,
    /// The account `list` (or `error`) belongs to; None until a load starts.
    pub loaded: Option<Account>,
    pub list: MyRepos,
    pub error: Option<ForgeError>,
    pub filter: String,
    /// The picked repository, an index into `list.repos`.
    pub picked: Option<usize>,
    /// Fill in the SSH clone URL rather than the https one.
    pub ssh: bool,
    /// The URL the last pick wrote into the form: the pick counts only while the URL is this.
    pub filled: String,
}

impl ServicePicker {
    pub fn selected(&self) -> Option<&Account> {
        self.accounts.get(self.account)
    }

    /// Whether the chosen account's list still has to be asked for.
    pub fn needs_load(&self) -> bool {
        self.selected().is_some_and(|a| !self.loading && self.loaded.as_ref() != Some(a))
    }

    /// The picked repository, while the form's `url` is still the one the pick filled in.
    pub fn picked_repo(&self, url: &str) -> Option<&HostedRepo> {
        self.picked.filter(|_| url == self.filled).and_then(|i| self.list.repos.get(i))
    }
}

/// The saved connections whose service can list the user's repositories: github.com first, then
/// in the order Preferences lists the services, then by host.
pub fn listing_accounts(connections: &[Connection]) -> Vec<Account> {
    let mut out: Vec<Account> = connections
        .iter()
        .map(|c| c.account.clone())
        .filter(|a| forge::lists_my_repos(a.provider) && forge::may_contact(a, connections))
        .collect();
    out.dedup();
    let rank = |a: &Account| {
        let public_github = a.provider == Provider::GitHub && a.host.eq_ignore_ascii_case("github.com");
        let order = Provider::ALL.iter().position(|p| *p == a.provider).unwrap_or(usize::MAX);
        (!public_github, order, a.host.to_lowercase())
    };
    out.sort_by_key(rank);
    out
}

/// "GitHub · github.com (ada)": the service, the account and the connection's user name.
pub fn account_label(a: &Account, connections: &[Connection]) -> String {
    let user = connections.iter().find(|c| c.account == *a).map(|c| c.user.trim()).filter(|u| !u.is_empty());
    match user {
        Some(u) => format!("{} · {} ({u})", a.provider.name(), a.display()),
        None => format!("{} · {}", a.provider.name(), a.display()),
    }
}

/// The repositories the filter keeps (indexes into `list.repos`): a case-insensitive match
/// anywhere in `owner/name`.
pub fn visible(p: &ServicePicker) -> Vec<usize> {
    let filter = p.filter.trim().to_lowercase();
    (0..p.list.repos.len()).filter(|i| p.list.repos[*i].full_name.to_lowercase().contains(&filter)).collect()
}

/// The clone URL to fill in: SSH or https as asked, else whichever the service sent.
pub fn url_for(r: &HostedRepo, ssh: bool) -> &str {
    let (first, second) = if ssh { (&r.ssh_url, &r.https_url) } else { (&r.https_url, &r.ssh_url) };
    if first.is_empty() { second } else { first }
}

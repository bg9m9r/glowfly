//! The "Publish to GitHub" dialog, offered when pushing a repository that has no remote.

use glowfly_core::forge::Account;
use std::path::PathBuf;

pub struct PublishForm {
    /// The repository being published and the branch to push.
    pub workdir: PathBuf,
    pub branch: String,
    /// GitHub accounts to create on (connections, or github.com when there are none).
    pub accounts: Vec<Account>,
    pub account: usize,
    /// The signed-in login on that account, once the owners have loaded.
    pub user: Option<String>,
    /// The user's login, then organizations.
    pub owners: Vec<String>,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub loading: bool,
    pub creating: bool,
    pub error: Option<String>,
    /// Whether the name was edited or Create tried, so its problem waits for them.
    pub touched: super::touched::Touched,
}

impl PublishForm {
    pub fn account(&self) -> Option<&Account> {
        self.accounts.get(self.account)
    }
}

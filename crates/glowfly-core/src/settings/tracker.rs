//! Where a repository's issues come from.

use crate::forge::Account;
use serde::{Deserialize, Serialize};

/// Where a repository's issues come from (This repository > Issue tracker).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum TrackerChoice {
    /// The code host's own issues (GitHub/GitLab/Bitbucket issues, Azure Boards).
    #[default]
    Auto,
    None,
    /// A connected Jira site, optionally limited to one project key.
    Jira {
        account: Account,
        project: String,
    },
    /// A connected Trello account, optionally limited to one board (empty: all boards).
    Trello {
        account: Account,
        #[serde(default)]
        board: String,
        #[serde(default)]
        board_name: String,
    },
}

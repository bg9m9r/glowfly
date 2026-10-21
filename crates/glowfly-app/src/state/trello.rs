//! The Board picker of Repo-specific preferences > Issue tracker, for a Trello tracker.

use glowfly_core::forge::{Account, TrelloBoard};

/// One account's open boards, loaded once per Preferences session (update/trello.rs).
#[derive(Default)]
pub struct BoardList {
    /// The account the list is (being) loaded for.
    pub account: Option<Account>,
    pub loading: bool,
    pub boards: Vec<TrelloBoard>,
    pub error: Option<String>,
}

impl BoardList {
    pub fn has(&self, id: &str) -> bool {
        self.boards.iter().any(|b| b.id == id)
    }
}

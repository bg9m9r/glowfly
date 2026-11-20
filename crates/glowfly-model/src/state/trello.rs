//! The Board picker of This repository > Issue tracker, for a Trello tracker, and Sign in with
//! Trello on Integrations > Trello.

use glowfly_core::forge::{Account, TrelloBoard};
use std::time::Instant;

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

/// Sign in with Trello on Integrations > Trello (update/trello_sign_in.rs).
pub struct TrelloSignIn {
    /// Glowfly's own API key; without one the button is hidden. `GLOWFLY_TRELLO_KEY`, which
    /// tests replace.
    pub key: Option<String>,
    pub step: SignInStep,
    /// The wait the runner may still run (when it started), from its start until its end
    /// arrives or it is stopped, whatever `step` shows.
    pub running: Option<Instant>,
    /// A Trello account was connected in this Preferences session: offer it as the open
    /// repository's tracker.
    pub offer_tracker: bool,
}

impl Default for TrelloSignIn {
    fn default() -> Self {
        Self {
            key: glowfly_core::trello_sign_in::GLOWFLY_TRELLO_KEY.map(str::to_owned),
            step: SignInStep::Idle,
            running: None,
            offer_tracker: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SignInStep {
    #[default]
    Idle,
    /// Waiting for Trello's answer since this instant (which tells this wait's end from an
    /// earlier one's).
    Waiting(Instant),
    /// The wait ended without a token. With `fallback`, the page offers Trello's page that shows
    /// a token to paste, with the key filled in.
    Failed { message: String, fallback: bool },
}

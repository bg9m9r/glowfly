//! The Board picker of This repository > Issue tracker, while Trello is the
//! repository's tracker: each account's boards load once per Preferences session.

use crate::effect::{AccountEffect, Effect};
use crate::state::trello::BoardList;
use crate::state::{AppState, PrefPage};
use glowfly_core::forge::{self, Account, TrelloBoard};
use glowfly_core::settings::TrackerChoice;

/// Opens the Issue Tracker page from the side list.
pub(super) fn show_page(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    p.page = PrefPage::RepoTracker;
    want_boards(st, fx);
}

/// Loads the boards of the repository's Trello account, unless they are loaded or loading.
pub(super) fn want_boards(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(w) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
    let TrackerChoice::Trello { account, .. } = st.settings.tracker_for(&w) else { return };
    if st.prefs.as_ref().is_some_and(|p| p.trello_boards.account.as_ref() != Some(&account)) {
        start(st, account, fx);
    }
}

/// "Try again": reloads `account`'s boards.
pub(super) fn load_boards(st: &mut AppState, account: Account, fx: &mut Vec<Effect>) {
    if st.prefs.is_some() {
        start(st, account, fx);
    }
}

fn start(st: &mut AppState, account: Account, fx: &mut Vec<Effect>) {
    let connected = forge::is_connected(&account, &st.settings.connections);
    let cwd = st.any_dir();
    let Some(p) = st.prefs.as_mut() else { return };
    if !connected {
        // Never contacted without a connection in this profile.
        p.trello_boards = BoardList { account: Some(account), error: Some("not connected to Trello".into()), ..Default::default() };
        return;
    }
    p.trello_boards = BoardList { account: Some(account.clone()), loading: true, boards: Vec::new(), error: None };
    fx.push(Effect::Account(AccountEffect::TrelloBoards { account, cwd }));
}

/// The boards arrived; a reply for an account no longer shown is ignored.
pub(super) fn boards_loaded(st: &mut AppState, account: Account, result: Result<Vec<TrelloBoard>, String>) {
    let Some(list) = st.prefs.as_mut().map(|p| &mut p.trello_boards).filter(|l| l.account.as_ref() == Some(&account)) else {
        return;
    };
    list.loading = false;
    match result {
        Ok(boards) => {
            list.boards = boards;
            list.error = None;
        }
        Err(e) => list.error = Some(e),
    }
}

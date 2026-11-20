//! Sign in with Trello on Integrations > Trello, with a key the test sets: signed out (the
//! button, and the manual fields collapsed under "Use your own API key"), waiting for Trello,
//! the fallback after the wait ran out, and the offer to use Trello for the open repository.

use super::snapshot_tests::{harness, remove_fixture, settings, with_repo};
use super::snapshot_tests_gitflow::pushed;
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::settings::{ThemeChoice, TrackerChoice};
use glowfly_model::msg::{ForgeMsg, Msg};
use glowfly_model::state::trello::SignInStep;
use glowfly_model::state::{AppState, PrefPage, PrefsState};
use std::time::Instant;

const KEY: &str = "0123456789abcdef0123456789abcdef";

fn acct() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

/// Preferences on Integrations > Trello with Glowfly's key set, at `step`.
fn prefs(step: SignInStep) -> PrefsState {
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    (prefs.int_provider, prefs.int_host) = (Provider::Trello, "trello.com".into());
    prefs.trello_sign_in.key = Some(KEY.into());
    prefs.trello_sign_in.step = step;
    prefs
}

fn signed_out(step: SignInStep) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.prefs = Some(prefs(step));
    st
}

#[test]
fn prefs_trello_sign_in() {
    let mut h = harness(signed_out(SignInStep::Idle), true);
    h.run();
    h.check_snapshot("prefs_trello_sign_in");
    assert!(h.query_by_label("Use your own API key").is_some());
    assert!(h.query_by_label("Get your API key").is_none(), "the manual fields start collapsed");
    h.get_by_label("Sign in with Trello").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Forge(ForgeMsg::StartTrelloSignIn))));
    h.get_by_label("Use your own API key").click();
    h.run();
    assert!(h.query_by_label("Get your API key").is_some(), "expanded");
}

#[test]
fn prefs_trello_sign_in_waiting() {
    let mut h = harness(signed_out(SignInStep::Waiting(Instant::now())), true);
    // The spinner keeps repainting.
    h.run_steps(4);
    h.check_snapshot("prefs_trello_sign_in_waiting");
    assert!(h.query_by_label("Sign in with Trello").is_none());
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert!(pushed(&h, &|m| matches!(m, Msg::Forge(ForgeMsg::CancelTrelloSignIn))));
}

#[test]
fn trello_sign_in_falls_back_to_the_token_page() {
    let message = "Glowfly stopped waiting for Trello after 5 minutes.".to_owned();
    let mut st = signed_out(SignInStep::Failed { message: message.clone(), fallback: true });
    st.prefs.as_mut().unwrap().int_user = KEY.into();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&message).is_some());
    assert!(h.query_by_label("Get your API key").is_some(), "the manual fields open");
    h.get_by_label("Get a token from Trello").click();
    h.run();
    let want = |m: &Msg| matches!(m, Msg::OpenUrl(u) if u.contains("scope=read,write&") && u.contains(KEY));
    assert!(pushed(&h, &want));
}

#[test]
fn prefs_trello_signed_in() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "trello-signed-in");
    st.settings.connections.push(Connection { account: acct(), user: "Ada Lovelace".into(), method: "access token".into() });
    let mut p = prefs(SignInStep::Idle);
    p.trello_sign_in.offer_tracker = true;
    st.prefs = Some(p);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("prefs_trello_signed_in");
    h.get_by_label("Use Trello for this repository").click();
    h.run();
    let all = |m: &Msg| {
        matches!(m, Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Trello { account, board, board_name }))
            if *account == acct() && board.is_empty() && board_name.is_empty())
    };
    assert!(pushed(&h, &all));
    drop(h);
    remove_fixture(&dir);
}

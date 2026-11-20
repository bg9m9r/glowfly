//! Sign in with Trello (update/trello_sign_in.rs): starting and cancelling the wait, the token
//! connecting the account as a typed one does, each way the wait ends without one, and the
//! offer to use Trello for the open repository.

use super::tests::state;
use super::update;
use crate::effect::partial::StopKey;
use crate::effect::{AccountEffect, ControlEffect, Effect};
use crate::msg::{ForgeMsg, Msg, PrefsMsg};
use crate::state::prefs::PrefsState;
use crate::state::trello::SignInStep;
use crate::state::{AppState, PrefPage};
use glowfly_core::forge::{Account, Provider};
use glowfly_core::trello_sign_in::SignInEnd;
use std::time::{Duration, Instant};

const KEY: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn trello() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

/// Preferences open on Integrations > Trello, with Glowfly's key set (or not).
fn on_trello(key: Option<&str>) -> AppState {
    let mut st = state();
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    prefs.int_provider = Provider::Trello;
    prefs.trello_sign_in.key = key.map(str::to_owned);
    st.prefs = Some(prefs);
    st
}

fn step(st: &AppState) -> &SignInStep {
    &st.prefs.as_ref().unwrap().trello_sign_in.step
}

fn stops(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Control(ControlEffect::StopGit(StopKey::TrelloSignIn)))).count()
}

/// Starts the wait at `at` and checks the effect it asks for.
fn start(st: &mut AppState, at: Instant) {
    let fx = update(st, Msg::Forge(ForgeMsg::StartTrelloSignIn), at);
    assert!(fx.iter().any(|e| matches!(e, Effect::Account(AccountEffect::TrelloSignIn { key, started }) if key == KEY && *started == at)));
    assert_eq!(stops(&fx), 1, "an earlier wait lets go of its port first");
    assert_eq!(step(st), &SignInStep::Waiting(at));
}

fn ended(st: &mut AppState, started: Instant, result: Result<String, SignInEnd>) -> Vec<Effect> {
    update(st, Msg::Forge(ForgeMsg::TrelloSignInEnded { started, key: KEY.into(), result }), started + Duration::from_secs(1))
}

#[test]
fn without_a_key_nothing_starts() {
    let mut st = on_trello(None);
    assert!(update(&mut st, Msg::Forge(ForgeMsg::StartTrelloSignIn), Instant::now()).is_empty());
    assert_eq!(step(&st), &SignInStep::Idle);
}

#[test]
fn a_token_connects_the_account_with_glowflys_key() {
    let mut st = on_trello(Some(KEY));
    let at = Instant::now();
    start(&mut st, at);
    assert!(update(&mut st, Msg::Forge(ForgeMsg::StartTrelloSignIn), at).is_empty(), "already waiting");
    let fx = ended(&mut st, at, Ok(TOKEN.into()));
    let connect = |e: &Effect| {
        matches!(e, Effect::Account(AccountEffect::Connect { account, user: Some(u), token: Some(t), .. })
            if *account == trello() && u == KEY && t == TOKEN)
    };
    assert!(fx.iter().any(connect), "verified and saved as a typed token is");
    let p = st.prefs.as_ref().unwrap();
    assert!(p.int_busy && p.trello_sign_in.step == SignInStep::Idle);
    let connected = ForgeMsg::Connected {
        account: trello(),
        method: "access token".into(),
        token: Some((Some(KEY.into()), TOKEN.into())),
        result: Ok("Ada".into()),
    };
    update(&mut st, Msg::Forge(connected), at);
    assert!(st.prefs.as_ref().unwrap().trello_sign_in.offer_tracker, "offers Trello for the open repository");
}

#[test]
fn cancel_stops_the_wait_and_its_end_is_dropped() {
    let mut st = on_trello(Some(KEY));
    let at = Instant::now();
    start(&mut st, at);
    let fx = update(&mut st, Msg::Forge(ForgeMsg::CancelTrelloSignIn), at);
    assert_eq!((stops(&fx), step(&st)), (1, &SignInStep::Idle));
    assert!(ended(&mut st, at, Ok(TOKEN.into())).is_empty(), "a token arriving after Cancel is not used");
    assert!(ended(&mut st, at, Err(SignInEnd::Cancelled)).is_empty());
    assert_eq!(step(&st), &SignInStep::Idle);
}

#[test]
fn an_earlier_waits_end_leaves_the_current_one() {
    let mut st = on_trello(Some(KEY));
    let first = Instant::now();
    let second = first + Duration::from_secs(3);
    start(&mut st, second);
    assert!(ended(&mut st, first, Err(SignInEnd::Cancelled)).is_empty());
    assert_eq!(step(&st), &SignInStep::Waiting(second));
}

#[test]
fn each_failure_says_why_and_offers_the_token_page() {
    let cases = [
        (SignInEnd::Denied, "Trello didn't allow Glowfly. Sign in again to try once more.", false),
        (SignInEnd::TimedOut, "Glowfly stopped waiting for Trello after 5 minutes.", true),
        (SignInEnd::NoPort, "Glowfly couldn't wait for Trello's answer: the local ports it uses are taken.", true),
        (SignInEnd::Failed("boom".into()), "Signing in with Trello failed: boom", true),
    ];
    for (end, message, fallback) in cases {
        let mut st = on_trello(Some(KEY));
        let at = Instant::now();
        start(&mut st, at);
        assert!(ended(&mut st, at, Err(end.clone())).is_empty(), "{end:?}");
        assert_eq!(step(&st), &SignInStep::Failed { message: message.into(), fallback }, "{end:?}");
        let user = &st.prefs.as_ref().unwrap().int_user;
        assert_eq!(user == KEY, fallback, "{end:?}: the token page's tokens are for Glowfly's key");
    }
}

#[test]
fn closing_preferences_stops_the_wait() {
    let mut st = on_trello(Some(KEY));
    start(&mut st, Instant::now());
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), Instant::now());
    assert!(st.prefs.is_none());
    assert_eq!(stops(&fx), 1);
}

#[test]
fn connecting_through_the_form_stops_the_wait() {
    let mut st = on_trello(Some(KEY));
    let at = Instant::now();
    start(&mut st, at);
    let connect = ForgeMsg::Connect { account: trello(), user: Some("mine".into()), token: Some(TOKEN.into()) };
    let fx = update(&mut st, Msg::Forge(connect), at);
    assert_eq!((stops(&fx), step(&st)), (1, &SignInStep::Idle), "stopped as Cancel stops it");
    assert!(ended(&mut st, at, Ok(TOKEN.into())).is_empty(), "the stopped wait's token is not used");
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), at);
    assert_eq!(stops(&fx), 0, "nothing left to stop");
}

#[test]
fn closing_preferences_stops_a_running_wait_whatever_the_page_shows() {
    let mut st = on_trello(Some(KEY));
    let at = Instant::now();
    start(&mut st, at);
    st.prefs.as_mut().unwrap().trello_sign_in.step = SignInStep::Idle;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), at);
    assert_eq!(stops(&fx), 1);
}

#[test]
fn the_fallback_keeps_a_key_already_typed() {
    let mut st = on_trello(Some(KEY));
    st.prefs.as_mut().unwrap().int_user = "my-own-key".into();
    let at = Instant::now();
    start(&mut st, at);
    ended(&mut st, at, Err(SignInEnd::TimedOut));
    assert!(matches!(step(&st), SignInStep::Failed { fallback: true, .. }));
    assert_eq!(st.prefs.as_ref().unwrap().int_user, "my-own-key");
}

//! Sign in with Trello on Integrations > Trello: the runner opens Trello's "Allow Glowfly" page
//! and waits for the token on a local port (`AccountEffect::TrelloSignIn`); a token is then
//! verified and saved as a typed one is (`ForgeMsg::Connect`). After a Trello account connects,
//! the page offers it as the open repository's tracker.

use crate::effect::partial::StopKey;
use crate::effect::{AccountEffect, ControlEffect, Effect};
use crate::msg::ForgeMsg;
use crate::state::AppState;
use crate::state::trello::{SignInStep, TrelloSignIn};
use glowfly_core::forge::{Account, Provider};
use glowfly_core::trello_sign_in::SignInEnd;
use std::time::Instant;

fn stop() -> Effect {
    Effect::Control(ControlEffect::StopGit(StopKey::TrelloSignIn))
}

/// "Sign in with Trello": waits for a token for Glowfly's key, unless already waiting.
pub(super) fn start(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(key) = p.trello_sign_in.key.clone() else { return };
    if matches!(p.trello_sign_in.step, SignInStep::Waiting(_)) {
        return;
    }
    p.trello_sign_in.step = SignInStep::Waiting(now);
    p.trello_sign_in.running = Some(now);
    p.int_error = None;
    // A wait left from an earlier Preferences session lets go of its port first.
    fx.push(stop());
    fx.push(Effect::Account(AccountEffect::TrelloSignIn { key, started: now }));
}

/// Cancel: stops waiting.
pub(super) fn cancel(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut() {
        stop_wait(&mut p.trello_sign_in, fx);
    }
}

/// A Trello account is being connected (through the manual form or otherwise): a wait still
/// running stops, as Cancel stops it.
pub(super) fn connecting(st: &mut AppState, account: &Account, fx: &mut Vec<Effect>) {
    if account.provider == Provider::Trello
        && let Some(p) = st.prefs.as_mut()
    {
        stop_wait(&mut p.trello_sign_in, fx);
    }
}

/// Preferences is closing: a wait still running stops, whatever the page shows.
pub(super) fn closing(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut() {
        stop_wait(&mut p.trello_sign_in, fx);
    }
}

/// Stops the wait the runner may still run and shows the page idle again; nothing when no
/// wait runs.
fn stop_wait(s: &mut TrelloSignIn, fx: &mut Vec<Effect>) {
    let waiting = matches!(s.step, SignInStep::Waiting(_));
    if s.running.take().is_some() || waiting {
        fx.push(stop());
    }
    if waiting {
        s.step = SignInStep::Idle;
    }
}

/// The wait begun at `started` ended. Only the wait the page shows counts: one cancelled,
/// replaced or left behind by a closed Preferences is dropped, its token unused.
pub(super) fn ended(
    st: &mut AppState,
    started: Instant,
    key: String,
    result: Result<String, SignInEnd>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    let Some(p) = st.prefs.as_mut() else { return };
    if p.trello_sign_in.running == Some(started) {
        p.trello_sign_in.running = None;
    }
    if p.trello_sign_in.step != SignInStep::Waiting(started) {
        return;
    }
    let failed = |message: String, fallback: bool| SignInStep::Failed { message, fallback };
    p.trello_sign_in.step = match &result {
        Ok(_) | Err(SignInEnd::Cancelled) => SignInStep::Idle,
        Err(SignInEnd::Denied) => failed(t.tr("prefs.trello.sign_in.denied").to_owned(), false),
        Err(SignInEnd::TimedOut) => failed(t.tr("prefs.trello.sign_in.timed_out").to_owned(), true),
        Err(SignInEnd::NoPort) => failed(t.tr("prefs.trello.sign_in.no_port").to_owned(), true),
        Err(SignInEnd::Failed(e)) => failed(t.trf("prefs.trello.sign_in.failed", &[("error", &e)]), true),
    };
    if matches!(p.trello_sign_in.step, SignInStep::Failed { fallback: true, .. }) && p.int_user.trim().is_empty() {
        // Trello's token page makes tokens for Glowfly's key: the manual form needs it, unless
        // the user already typed a key of their own there.
        p.int_user.clone_from(&key);
    }
    if let Ok(token) = result {
        let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
        super::forge::handle(st, ForgeMsg::Connect { account, user: Some(key), token: Some(token) }, now, fx);
    }
}

/// A Trello account connected (signed in or typed): a wait still running stops, and the page
/// offers the account as the open repository's tracker.
pub(super) fn connected(st: &mut AppState, account: &Account, fx: &mut Vec<Effect>) {
    if account.provider == Provider::Trello
        && let Some(p) = st.prefs.as_mut()
    {
        stop_wait(&mut p.trello_sign_in, fx);
        p.trello_sign_in.step = SignInStep::Idle;
        p.trello_sign_in.offer_tracker = true;
    }
}

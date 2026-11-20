//! Sign in with Trello on Integrations > Trello: the runner opens Trello's "Allow Glowfly" page
//! and waits for the token on a local port (`AccountEffect::TrelloSignIn`); a token is then
//! verified and saved as a typed one is (`ForgeMsg::Connect`). After a Trello account connects,
//! the page offers it as the open repository's tracker.

use crate::effect::partial::StopKey;
use crate::effect::{AccountEffect, ControlEffect, Effect};
use crate::msg::ForgeMsg;
use crate::state::AppState;
use crate::state::trello::SignInStep;
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
    p.int_error = None;
    // A wait left from an earlier Preferences session lets go of its port first.
    fx.push(stop());
    fx.push(Effect::Account(AccountEffect::TrelloSignIn { key, started: now }));
}

/// Cancel: stops waiting.
pub(super) fn cancel(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut()
        && matches!(p.trello_sign_in.step, SignInStep::Waiting(_))
    {
        p.trello_sign_in.step = SignInStep::Idle;
        fx.push(stop());
    }
}

/// Preferences is closing: a wait still running stops.
pub(super) fn closing(st: &AppState, fx: &mut Vec<Effect>) {
    if st.prefs.as_ref().is_some_and(|p| matches!(p.trello_sign_in.step, SignInStep::Waiting(_))) {
        fx.push(stop());
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
    let Some(p) = st.prefs.as_mut().filter(|p| p.trello_sign_in.step == SignInStep::Waiting(started)) else { return };
    let failed = |message: String, fallback: bool| SignInStep::Failed { message, fallback };
    p.trello_sign_in.step = match &result {
        Ok(_) | Err(SignInEnd::Cancelled) => SignInStep::Idle,
        Err(SignInEnd::Denied) => failed(t.tr("prefs.trello.sign_in.denied").to_owned(), false),
        Err(SignInEnd::TimedOut) => failed(t.tr("prefs.trello.sign_in.timed_out").to_owned(), true),
        Err(SignInEnd::NoPort) => failed(t.tr("prefs.trello.sign_in.no_port").to_owned(), true),
        Err(SignInEnd::Failed(e)) => failed(t.trf("prefs.trello.sign_in.failed", &[("error", &e)]), true),
    };
    if matches!(p.trello_sign_in.step, SignInStep::Failed { fallback: true, .. }) {
        // Trello's token page makes tokens for Glowfly's key: the manual form needs it.
        p.int_user.clone_from(&key);
    }
    if let Ok(token) = result {
        let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
        super::forge::handle(st, ForgeMsg::Connect { account, user: Some(key), token: Some(token) }, now, fx);
    }
}

/// A Trello account connected (signed in or typed): offer it as the open repository's tracker.
pub(super) fn connected(st: &mut AppState, account: &Account) {
    if account.provider == Provider::Trello
        && let Some(p) = st.prefs.as_mut()
    {
        p.trello_sign_in.offer_tracker = true;
        p.trello_sign_in.step = SignInStep::Idle;
    }
}

//! Sign in with Trello (`AccountEffect::TrelloSignIn`): opens Trello's "Allow Glowfly" page
//! through `Msg::OpenUrl` once the local port is bound (not when Cancel came first), then waits
//! for the token until Trello answers, the time runs out or Cancel stops it
//! (`StopKey::TrelloSignIn`).

use super::execute::Cx;
use glowfly_core::trello_sign_in::{self, TIMEOUT};
use glowfly_model::msg::{ForgeMsg, Msg};
use std::time::Instant;

pub(super) fn sign_in(key: String, started: Instant, cx: &Cx<'_>) -> Msg {
    let stopped = || cx.stop.is_some_and(glowfly_core::gitcli::Stop::requested);
    let result = trello_sign_in::wait_for_token(&key, TIMEOUT, &stopped, |url| (cx.send)(Msg::OpenUrl(url)));
    tracing::debug!(ok = result.is_ok(), "trello sign-in ended");
    Msg::Forge(ForgeMsg::TrelloSignInEnded { started, key, result })
}

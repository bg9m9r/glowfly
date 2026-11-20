//! Sign in with Trello: Trello's "Allow Glowfly" page hands a token for Glowfly's own API key
//! back to the app, without the user copying it.
//!
//! 1. [`wait_for_token`] binds the first free port of [`PORTS`] on `127.0.0.1` and `[::1]`
//!    (browsers may resolve `localhost` to either), picks a random nonce and hands the
//!    authorize URL, whose `return_url` is `http://localhost:<port>/trello/<nonce>`, to the
//!    caller to open in the browser. Trello allows only return URLs whose origin is listed for
//!    the key, so the ports are fixed.
//! 2. Once the user allows Glowfly, Trello sends the browser to the return URL with the token
//!    in the fragment, which never reaches a server. The page served there
//!    ([`request::PAGE`]) posts the fragment back to `/trello/<nonce>/token`.
//! 3. The listener answers only those two paths with the exact nonce and a local `Host`;
//!    anything else gets a 404 and it keeps waiting. It ends with the token, Trello's refusal,
//!    after [`TIMEOUT`], when the caller stops it, or when no port is free.
//!
//! The API key may be public; only the token is secret, and it is never logged.

mod listener;
pub(crate) mod request;

#[cfg(test)]
mod listener_tests;
#[cfg(test)]
mod request_tests;

use std::time::Duration;

pub use listener::wait_for_token;

/// Glowfly's own Trello API key, for Sign in with Trello; None hides the button (the owner
/// sets it once the Glowfly app exists on Trello, with [`PORTS`] as its allowed origins).
pub const GLOWFLY_TRELLO_KEY: Option<&str> = None;

/// The local ports Trello may return to, tried in order: each `http://localhost:<port>` is an
/// allowed origin of [`GLOWFLY_TRELLO_KEY`].
pub const PORTS: [u16; 3] = [47613, 47614, 47615];

/// How long the listener waits for Trello's answer.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// Why Sign in with Trello ended without a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInEnd {
    /// The user didn't allow Glowfly on Trello's page.
    Denied,
    /// No answer came within the timeout.
    TimedOut,
    /// The caller stopped waiting (Cancel).
    Cancelled,
    /// Every port of [`PORTS`] was in use.
    NoPort,
    /// Anything else, in words: a key that isn't one, or the listener failing.
    Failed(String),
}

/// What the browser posted back: the URL fragment Trello returned to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Fragment {
    Token(String),
    Denied,
    /// Not a token or a refusal: ignored, the listener keeps waiting.
    Invalid,
}

/// Reads the fragment Trello returned to (with or without its `#`): `token=<token>`, possibly
/// among other `&`-separated parameters, or a bare token. An empty token or an `error`
/// parameter is a refusal. Trello doesn't document the format, so this reads it tolerantly.
pub(crate) fn read_fragment(raw: &str) -> Fragment {
    let raw = raw.trim().trim_start_matches('#');
    if !raw.contains('=') {
        return if is_token(raw) { Fragment::Token(raw.to_owned()) } else { Fragment::Invalid };
    }
    let mut token = None;
    let mut error = false;
    for (name, value) in raw.split('&').filter_map(|p| p.split_once('=')) {
        match name {
            "token" => token = Some(value),
            "error" => error = true,
            _ => {}
        }
    }
    match (token, error) {
        (Some(t), false) if is_token(t) => Fragment::Token(t.to_owned()),
        (Some(""), _) | (_, true) => Fragment::Denied,
        _ => Fragment::Invalid,
    }
}

/// A token's shape: ASCII letters and digits, 32 to 256 of them.
pub(crate) fn is_token(s: &str) -> bool {
    (32..=256).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

//! Credential lookups for hosting-service calls: one lookup in flight per account, folder and
//! kind (`SignInFlights`), and reporting each answer to the model's shared cache
//! (`ForgeMsg::AuthFound`).
//!
//! Every call that finds no credential in its request asks here. Requests that arrive while a
//! lookup for the same key runs wait for it and get its answer or its failure, so Git Credential
//! Manager is asked (and shows its window) once. Nothing is kept after the flight ends: the
//! model's cache holds the answers from then on.

use super::execute::Cx;
use glowfly_core::error::{Error, Result};
use glowfly_core::forge::{self, Account, Auth};
use glowfly_core::paths::same_spelling;
use glowfly_model::effect::FoundAuth;
use glowfly_model::msg::{ForgeMsg, Msg};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

/// What decides whether two lookups would find the same answer. Interactive and quiet lookups
/// are apart (a background request never waits behind a sign-in window, and a Sign in never
/// takes a quiet failure), and so are connected and unconnected ones (the saved token counts only
/// for the connected).
#[derive(Clone, Debug)]
pub(crate) struct FlightKey {
    pub account: String,
    pub cwd: PathBuf,
    pub interactive: bool,
    pub connected: bool,
}

impl PartialEq for FlightKey {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account
            && self.interactive == other.interactive
            && self.connected == other.connected
            && same_spelling(&self.cwd, &other.cwd)
    }
}

/// What a lookup answers with. A failure is the text of an `Error::Auth` (every failure of
/// `resolve_auth` is one), so rebuilding it keeps `ForgeError::auth` true and the runner's
/// "authentication failed: ..." wording the same.
type Answer = std::result::Result<Auth, String>;

struct Flight {
    answer: Mutex<Option<Answer>>,
    ready: Condvar,
}

/// The lookups running now, shared by every worker.
pub struct SignInFlights {
    flights: Mutex<Vec<(FlightKey, Arc<Flight>)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Held by the lookup that leads a flight: when it ends, even by a panic, the key is freed and
/// the waiters are answered.
struct Leading<'a> {
    flights: &'a SignInFlights,
    flight: &'a Arc<Flight>,
}

impl Drop for Leading<'_> {
    fn drop(&mut self) {
        lock(&self.flights.flights).retain(|(_, f)| !Arc::ptr_eq(f, self.flight));
        let mut answer = lock(&self.flight.answer);
        if answer.is_none() {
            *answer = Some(Err(forge::lookup_stopped_text()));
        }
        drop(answer);
        self.flight.ready.notify_all();
    }
}

impl SignInFlights {
    pub const fn new() -> Self {
        Self { flights: Mutex::new(Vec::new()) }
    }

    /// Runs `lookup` unless one for an equal key is running, and then waits for that one's answer.
    pub(crate) fn join(&self, key: FlightKey, lookup: impl FnOnce() -> Answer) -> Answer {
        let (flight, leads) = {
            let mut table = lock(&self.flights);
            match table.iter().find(|(k, _)| *k == key) {
                Some((_, f)) => (Arc::clone(f), false),
                None => {
                    let f = Arc::new(Flight { answer: Mutex::new(None), ready: Condvar::new() });
                    table.push((key.clone(), Arc::clone(&f)));
                    (f, true)
                }
            }
        };
        if !leads {
            tracing::debug!("sign-in for {}: waited for the lookup in flight", key.account);
            let waiting = flight.ready.wait_while(lock(&flight.answer), |a| a.is_none()).unwrap_or_else(PoisonError::into_inner);
            return waiting.clone().unwrap_or_else(|| Err(forge::lookup_stopped_text()));
        }
        let leading = Leading { flights: self, flight: &flight };
        let answer = lookup();
        *lock(&flight.answer) = Some(answer.clone());
        drop(leading);
        answer
    }

    /// How many requests wait on the flight for `key` (tests).
    #[cfg(test)]
    fn waiting(&self, key: &FlightKey) -> usize {
        // The table's and the leader's references are not waiters.
        lock(&self.flights).iter().find(|(k, _)| k == key).map_or(0, |(_, f)| Arc::strong_count(f).saturating_sub(2))
    }
}

impl Default for SignInFlights {
    fn default() -> Self {
        Self::new()
    }
}

/// One credential lookup a call needs.
pub(super) struct Ask<'a> {
    pub account: Account,
    /// The folder the lookup runs in (git reads that folder's credential helpers).
    pub cwd: &'a Path,
    pub interactive: bool,
    pub connected: bool,
    /// Its own lookup reads only the environment: a background call of an unconnected account.
    pub env_only: bool,
    /// The user asked for this call (Sign in, or their own action rather than a background one).
    pub asked: bool,
}

impl SignInFlights {
    /// Finds the credential for `ask`: through the flight for its key, or from the environment
    /// alone when that is all the request may read.
    pub(super) fn lookup(&self, ask: &Ask<'_>) -> Result<Auth> {
        if ask.env_only {
            return forge::background_auth(&ask.account, ask.cwd, false);
        }
        let key = FlightKey { account: ask.account.key(), cwd: ask.cwd.to_owned(), interactive: ask.interactive, connected: ask.connected };
        self.join(key, || forge::resolve_auth(&ask.account, ask.cwd, ask.interactive, ask.connected).map_err(auth_text))
            .map_err(Error::Auth)
    }
}

/// A sign-in failure as the user reads it: "not signed in to GitHub", not "authentication failed: ...".
pub(super) fn auth_text(e: Error) -> String {
    match e {
        Error::Auth(m) => m,
        e => e.to_string(),
    }
}

/// The message that hands a found credential to the model's cache.
pub(super) fn found(ask: &Ask<'_>, auth: &Auth) -> Msg {
    Msg::Forge(ForgeMsg::AuthFound(Box::new(FoundAuth {
        account: ask.account.clone(),
        dir: ask.cwd.to_owned(),
        connected: ask.connected,
        asked: ask.asked,
        auth: auth.clone(),
    })))
}

/// Looks the credential up and, when found, sends it to the model before the call's own result.
/// Every request that shared the flight reports it: storing it again changes nothing.
pub(super) fn sign_in(cx: &Cx<'_>, ask: Ask<'_>) -> Result<Auth> {
    let auth = cx.sign_ins.lookup(&ask)?;
    (cx.send)(found(&ask, &auth));
    Ok(auth)
}

#[cfg(test)]
#[path = "sign_ins_tests.rs"]
mod tests;

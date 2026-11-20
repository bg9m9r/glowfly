//! The hosting-service credentials Glowfly holds in memory: one answer per account, shared by
//! every tab and the dashboard (global, never parked with a tab).
//!
//! Where an answer can be reused depends on where it came from. An environment token or a saved
//! token is the same in every folder (`resolve_auth` finds them first, wherever it runs). An
//! answer from a git credential helper depends on the folder's git configuration, so it is
//! reused only in the folder the lookup ran in.

use glowfly_core::forge::Auth;
use glowfly_core::paths::same_spelling;
use std::path::{Path, PathBuf};

enum Place {
    Anywhere,
    Folder(PathBuf),
}

struct Entry {
    account: String,
    place: Place,
    auth: Auth,
    /// The user asked for it in this folder, or the account is connected: a background refresh of
    /// an unconnected account may keep using it.
    background_ok: bool,
}

impl Entry {
    fn is(&self, account: &str, place: &Place) -> bool {
        self.account == account
            && match (&self.place, place) {
                (Place::Anywhere, Place::Anywhere) => true,
                (Place::Folder(a), Place::Folder(b)) => same_spelling(a, b),
                _ => false,
            }
    }
}

#[derive(Default)]
pub struct AuthCache {
    entries: Vec<Entry>,
}

impl AuthCache {
    /// The answer a lookup for `account` (a key) in `dir` would reuse. `env_only`: the request's
    /// own lookup reads only the environment (a background call of an unconnected account), so
    /// only an `Anywhere` answer or one with `background_ok` serves it. `Anywhere` is checked
    /// before a folder's (`resolve_auth`'s order: environment, saved token, then helper).
    pub fn get(&self, account: &str, dir: &Path, env_only: bool) -> Option<&Auth> {
        let anywhere = self.entries.iter().find(|e| e.account == account && matches!(e.place, Place::Anywhere));
        let in_dir = || {
            self.entries.iter().find(|e| {
                e.account == account && matches!(&e.place, Place::Folder(d) if same_spelling(d, dir)) && (!env_only || e.background_ok)
            })
        };
        anywhere.or_else(in_dir).map(|e| &e.auth)
    }

    /// Stores `auth`. A helper answer goes under its folder, anything else under `Anywhere`. It
    /// replaces the answer for the same account and place, keeping `background_ok` if that one
    /// had it.
    pub fn store(&mut self, account: String, dir: &Path, auth: Auth, background_ok: bool) {
        let place = if auth.is_helper_answer() { Place::Folder(dir.to_owned()) } else { Place::Anywhere };
        match self.entries.iter_mut().find(|e| e.is(&account, &place)) {
            Some(e) => (e.background_ok, e.auth) = (e.background_ok || background_ok, auth),
            None => self.entries.push(Entry { account, place, auth, background_ok }),
        }
    }

    /// Drops every answer for the account, in every folder (a rejected sign-in, a connection
    /// change).
    pub fn forget(&mut self, account: &str) {
        self.entries.retain(|e| e.account != account);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
#[path = "auth_cache_tests.rs"]
mod tests;

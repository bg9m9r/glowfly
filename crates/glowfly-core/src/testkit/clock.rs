//! Pinned dates for a fixture folder: the exploration tests need commit ids that are the same on
//! every run, so every git command gitcli starts inside a pinned root gets a fixed, rising date.
//! Glowfly's own signatures (`undo::glowfly_signature`) follow the same clock.
//!
//! Test and `testkit` builds only: a release build compiles none of this.

use crate::paths::is_within;
use git2::{Error, Signature, Time};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Seconds between two dated commands.
const STEP: i64 = 60;

/// Each pinned root and the date its next command gets.
static ROOTS: Mutex<BTreeMap<PathBuf, i64>> = Mutex::new(BTreeMap::new());

fn roots() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, i64>> {
    ROOTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Gives a command started in `cwd` the next date when `cwd` is in a pinned root, for each of the
/// two dates the caller's `env` doesn't name (so a caller's value, an `UNSET` too, always wins).
pub(crate) fn apply(cmd: &mut Command, cwd: &Path, env: &[(&str, &str)]) {
    let named = |var: &str| env.iter().any(|(k, _)| *k == var);
    let missing: Vec<&str> = ["GIT_AUTHOR_DATE", "GIT_COMMITTER_DATE"].into_iter().filter(|v| !named(v)).collect();
    if missing.is_empty() {
        return;
    }
    if let Some(at) = next_for(cwd) {
        for var in missing {
            cmd.env(var, format!("@{at} +0000"));
        }
    }
}

/// Dates every git command started inside `root` from `start` (Unix seconds), `STEP` apart,
/// unless the caller sets its own dates. Pinning a root again restarts its clock.
pub fn pin(root: &Path, start: i64) {
    roots().insert(root.to_path_buf(), start);
}

/// Stops pinning `root`.
pub fn unpin(root: &Path) {
    roots().remove(root);
}

/// The next date for a command in `path`, when `path` is inside a pinned root. Roots are matched
/// by spelling ([`is_within`]), so `\?\C:\x` and `C:\x` are the same root.
pub(crate) fn next_for(path: &Path) -> Option<i64> {
    let mut roots = roots();
    let tick = roots.iter_mut().find(|(root, _)| is_within(path, root)).map(|(_, tick)| tick)?;
    let at = *tick;
    *tick += STEP;
    Some(at)
}

/// A signature dated by [`next_for`] when `path` is pinned, else now.
pub(crate) fn signature(path: &Path, name: &str, email: &str) -> Result<Signature<'static>, Error> {
    match next_for(path) {
        Some(at) => Signature::new(name, email, &Time::new(at, 0)),
        None => Signature::now(name, email),
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;

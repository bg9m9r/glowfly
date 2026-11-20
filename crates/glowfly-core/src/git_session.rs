//! What the git children Glowfly runs say about themselves, asked once per program per session:
//! `git version` and `git --exec-path`. Keyed by the chosen program ([`git_program::chosen`];
//! None is the git on the PATH, which is fixed at start), so choosing another git in Preferences
//! asks that one. Only answers are kept: a git that could not run is asked again next time.
//! Both are builtins that don't look for a repository, so a cached answer can't differ by `cwd`.

use crate::git_program;
use crate::git_version::Version;
use crate::gitcli;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

type Cache<T> = Mutex<BTreeMap<Option<PathBuf>, T>>;

static VERSIONS: Cache<Version> = Mutex::new(BTreeMap::new());
static EXEC_PATHS: Cache<PathBuf> = Mutex::new(BTreeMap::new());

fn locked<T>(cache: &Cache<T>) -> std::sync::MutexGuard<'_, BTreeMap<Option<PathBuf>, T>> {
    cache.lock().unwrap_or_else(|e| e.into_inner())
}

/// The answer kept for `key()`, else `ask()`'s. `ask` runs without the lock held (starting git
/// is slow and must not block other workers). Its answer is stored only when it is Some and the
/// key is still the same (a switch while git was being asked doesn't store under the old key).
fn remember<T: Clone>(cache: &Cache<T>, key: &dyn Fn() -> Option<PathBuf>, ask: impl FnOnce() -> Option<T>) -> Option<T> {
    let k = key();
    if let Some(v) = locked(cache).get(&k) {
        return Some(v.clone());
    }
    let answer = ask()?;
    if key() == k {
        locked(cache).insert(k, answer.clone());
    }
    Some(answer)
}

/// The chosen git's version (`git version`), asked once.
pub(crate) fn version(cwd: &Path) -> Option<Version> {
    remember(&VERSIONS, &git_program::chosen, || gitcli::parse_version(&gitcli::read(cwd, &gitcli::args(&["version"])).stdout))
}

/// The chosen git's exec path (`git --exec-path`), asked once.
pub(crate) fn exec_path(cwd: &Path) -> Option<PathBuf> {
    remember(&EXEC_PATHS, &git_program::chosen, || {
        let out = gitcli::read(cwd, &gitcli::args(&["--exec-path"]));
        let path = out.stdout.trim();
        (out.ok && !path.is_empty()).then(|| PathBuf::from(path))
    })
}

/// Keeps `v` as `program`'s version, replacing an older answer (a fresh probe of it).
pub(crate) fn note_version(program: Option<PathBuf>, v: Version) {
    locked(&VERSIONS).insert(program, v);
}

#[cfg(test)]
#[path = "git_session_tests.rs"]
mod tests;

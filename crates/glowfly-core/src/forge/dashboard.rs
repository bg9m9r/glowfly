//! Reading local repositories' hosting targets and asking several services at once, for the
//! cross-repository dashboard. A repository is opened with libgit2 only (its remote URLs, as
//! libgit2 rewrites them with `insteadOf`), so nothing in it runs; the services are then asked
//! with a bounded number of calls in flight.

use super::filters::{Builtin, Me, matching};
use super::{Auth, Connection, Issue, PullRequest, Target, parse_remote};
use crate::error::{Error, Result};
use crate::paths::{self, FolderId};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The most repositories read, or service calls made, at once.
pub const CONCURRENCY: usize = 4;

/// A remote whose URL names a repository (or site) on a hosting service.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteTarget {
    pub remote: String,
    pub url: String,
    pub target: Target,
}

/// What the dashboard needs of one local repository.
#[derive(Clone, Debug)]
pub struct ScannedRepo {
    /// The work tree, as libgit2 reports it.
    pub workdir: PathBuf,
    /// Where it really is on disk (`paths::resolve`), the key of its saved settings.
    pub real_workdir: PathBuf,
    pub folder: Option<FolderId>,
    pub name: String,
    /// Every recognized remote, origin first, each target once.
    pub remotes: Vec<RemoteTarget>,
    /// The code host among `remotes`, picked as a tab picks it (origin first).
    pub host: Option<usize>,
}

/// Opens the repository at (or above) `path` with libgit2 and reads its hosting targets.
pub fn read_repo(path: &Path, connections: &[Connection]) -> Result<ScannedRepo> {
    let repo = git2::Repository::discover(path).map_err(|e| crate::safe_directory::open_error(path, &e))?;
    if repo.is_bare() {
        return Err(Error::Unsupported("bare repositories are not supported".into()));
    }
    let workdir = repo.workdir().ok_or_else(|| Error::Unsupported("repository has no working directory".into()))?.to_path_buf();
    let real_workdir = paths::resolve(&workdir);
    let folder = FolderId::of(&workdir);
    let name = workdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| workdir.display().to_string());
    let names = repo.remotes()?;
    let urls: Vec<(String, String)> =
        names.iter().flatten().filter_map(|r| Some((r.to_owned(), repo.find_remote(r).ok()?.url()?.to_owned()))).collect();
    let remotes = recognized(&urls, connections);
    let host = code_host(&urls, connections).and_then(|(_, t)| remotes.iter().position(|r| r.target == t));
    Ok(ScannedRepo { workdir, real_workdir, folder, name, remotes, host })
}

/// A short reason a folder couldn't be read, for a list that already names the folder.
pub fn unreadable_reason(e: &Error) -> String {
    match e {
        Error::NotARepository(_) => "not a Git repository".into(),
        Error::NotOwned(_) => "owned by another user".into(),
        Error::Unsupported(m) => m.clone(),
        e => e.to_string(),
    }
}

/// Every remote URL `parse_remote` recognizes, origin first and then in the order given, each
/// target once (two remotes for one repository, say over https and ssh, collapse).
pub fn recognized(remote_urls: &[(String, String)], connections: &[Connection]) -> Vec<RemoteTarget> {
    let mut found: Vec<RemoteTarget> = remote_urls
        .iter()
        .filter_map(|(name, url)| {
            parse_remote(url, connections).map(|target| RemoteTarget { remote: name.clone(), url: url.clone(), target })
        })
        .collect();
    // Stable: the rest keep their order.
    found.sort_by_key(|r| r.remote != "origin");
    let mut out: Vec<RemoteTarget> = Vec::new();
    for r in found {
        if !out.iter().any(|o| o.target == r.target) {
            out.push(r);
        }
    }
    out
}

/// The code host of a repository with these remotes: origin when it is recognized, else the
/// first recognized remote. Returns the remote's name and its target.
pub fn code_host(remote_urls: &[(String, String)], connections: &[Connection]) -> Option<(String, Target)> {
    remote_urls
        .iter()
        .filter_map(|(name, url)| parse_remote(url, connections).map(|t| (name.clone(), t)))
        .min_by_key(|(name, _)| name != "origin")
}

/// Runs `f` on every item with at most `limit` (at least one) running at once, and returns the
/// results in the order of `items`.
pub fn run_bounded<T: Send, R: Send>(items: Vec<T>, limit: usize, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }
    let inputs: Vec<Mutex<Option<T>>> = items.into_iter().map(|t| Mutex::new(Some(t))).collect();
    let outputs: Vec<Mutex<Option<R>>> = (0..n).map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let take = |i: usize| inputs[i].lock().unwrap_or_else(|e| e.into_inner()).take();
    std::thread::scope(|s| {
        for _ in 0..limit.max(1).min(n) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= n {
                        break;
                    }
                    if let Some(item) = take(i) {
                        let r = f(item);
                        *outputs[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                    }
                }
            });
        }
    });
    // Every index was taken by exactly one worker, and a panic in `f` ends the scope above.
    outputs.into_iter().filter_map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner())).collect()
}

/// What one target answered; `None` for what wasn't asked.
#[derive(Debug)]
pub struct Fetched {
    pub prs: Option<Result<Vec<PullRequest>>>,
    pub issues: Option<Result<Vec<Issue>>>,
}

/// Asks `t` for its open pull requests (`prs`) and the issues assigned to the signed-in user
/// (`issues`).
pub fn fetch_target(t: &Target, auth: &Auth, prs: bool, issues: bool) -> Fetched {
    Fetched { prs: prs.then(|| super::pull_requests(t, auth)), issues: issues.then(|| super::my_issues(t, auth)) }
}

/// The pull requests `me` opened, and those waiting for the review of `me` or a team `me`
/// belongs to, in order.
pub fn mine_and_review<'a>(prs: &'a [PullRequest], me: &Me) -> (Vec<&'a PullRequest>, Vec<&'a PullRequest>) {
    let pick = |b: Builtin| matching::matching_prs(prs, &b.conditions(), Some(me)).unwrap_or_default();
    (pick(Builtin::MyPrs), pick(Builtin::NeedsMyReview))
}

#[cfg(test)]
#[path = "dashboard_tests.rs"]
mod tests;

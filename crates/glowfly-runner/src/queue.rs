//! One queue per repository for the effects that change it (index, working tree, refs), so a
//! stage, discard or conflict save never runs at the same time as a pull, a checkout or another
//! edit of the same repository: each runs after the ones queued before it. Reads (loading
//! status, diffs, history) and network-only jobs (fetch, push) don't queue; which effects queue,
//! and on which repository, is `Effect::queue` (each area's `EffectMeta`).
//!
//! A queue belongs to a folder, not to a spelling of its path: jobs for `C:/x/repo/`, a junction
//! to it, or `REPO` on a volume that ignores case share one worker. Telling them apart takes the
//! disk (`paths::FolderId`), so `submit` hands each job to a router thread that looks the folder
//! up (once per spelling) and passes the job on, in order; the UI thread never waits on it.

use glowfly_core::paths::FolderId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

pub(super) type Job = Box<dyn FnOnce() + Send>;

/// What a queue is keyed by: the folder's identity, or (for a path with none: missing, or on a
/// volume without file ids) its resolved spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Folder(FolderId),
    Spelling(PathBuf),
}

/// Finds the key of a repository path; `identify` in production, a stand-in in tests.
type Identify = fn(&Path) -> Key;

fn identify(repo: &Path) -> Key {
    match FolderId::of(repo) {
        Some(id) => Key::Folder(id),
        None => Key::Spelling(glowfly_core::paths::resolve(repo).components().collect()),
    }
}

pub(super) struct RepoQueues {
    /// The router thread, started on first use.
    router: Mutex<Option<Sender<(PathBuf, Job)>>>,
    identify: Identify,
}

impl Default for RepoQueues {
    fn default() -> Self {
        Self { router: Mutex::new(None), identify }
    }
}

impl RepoQueues {
    /// Queues `job` behind the earlier jobs for the folder `repo` names. Returns the job back if
    /// no thread could be started for it.
    pub(super) fn submit(&self, repo: &Path, job: Job) -> Result<(), Job> {
        let mut router = self.router.lock().unwrap_or_else(|p| p.into_inner());
        let mut job = job;
        if let Some(tx) = router.as_ref() {
            match tx.send((repo.to_owned(), job)) {
                Ok(()) => return Ok(()),
                Err(mpsc::SendError((_, back))) => job = back, // the router is gone; start another
            }
        }
        let (tx, rx) = mpsc::channel::<(PathBuf, Job)>();
        let identify = self.identify;
        let started = std::thread::Builder::new().name("glowfly-repo-queues".into()).spawn(move || route(rx, identify));
        if started.is_err() {
            return Err(job);
        }
        // The router is alive, so the send can't fail.
        let _ = tx.send((repo.to_owned(), job));
        *router = Some(tx);
        Ok(())
    }
}

/// Passes each job to its folder's worker, starting workers on first use. A job whose worker
/// can't be started runs on a thread of its own, or here as a last resort.
fn route(rx: mpsc::Receiver<(PathBuf, Job)>, identify: Identify) {
    let mut keys: HashMap<PathBuf, Key> = HashMap::new();
    let mut workers: HashMap<Key, Sender<Job>> = HashMap::new();
    for (repo, job) in rx {
        let key = keys.entry(repo.clone()).or_insert_with(|| identify(&repo)).clone();
        let mut job = job;
        if let Some(tx) = workers.get(&key) {
            match tx.send(job) {
                Ok(()) => continue,
                Err(mpsc::SendError(back)) => job = back, // its worker is gone; start another
            }
        }
        let (tx, worker_rx) = mpsc::channel::<Job>();
        let name = format!("glowfly-repo-{}", repo.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
        let started = std::thread::Builder::new().name(name).spawn(move || {
            for job in worker_rx {
                job();
            }
        });
        if started.is_err() {
            tracing::error!("could not start a queue for {}", repo.display());
            if let Err(e) = std::thread::Builder::new().name("glowfly-repo-job".into()).spawn(job) {
                tracing::error!("could not start a thread for a job: {e}");
            }
            continue;
        }
        // The worker is alive, so the send can't fail.
        let _ = tx.send(job);
        workers.insert(key, tx);
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;

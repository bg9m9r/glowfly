//! The whole-history search index's builds (`IndexEffect`). One "glowfly-index" thread works
//! through them, one at a time across every tab and the newest request first: a build for a
//! repository replaces that repository's waiting one and stops its running one, and Forget stops
//! both. A stopped build reports nothing; its tab has asked for a newer one or no longer wants it.
//! With the store attached (`index_disk.rs`), a build without a base reads the repository's saved
//! index first, and a finished build is saved after it is reported, before the next request.
//! Queries go to their own worker (`index_query.rs`).

use super::Waker;
use super::index_disk::Disk;
use super::index_query::{Haystack, QueryRequest, QueryWorker};
use crate::effect::IndexEffect;
use crate::msg::{Msg, SearchMsg};
use glowfly_core::commit_index::{self, CommitIndex, IndexKey};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

pub(super) struct BuildRequest {
    workdir: PathBuf,
    build: u64,
    key: IndexKey,
    base: Option<Arc<CommitIndex>>,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
struct Builds {
    /// The repository whose build is running, and that build's stop.
    running: Option<(PathBuf, Arc<AtomicBool>)>,
    /// Builds not started yet, oldest first; the newest starts next.
    waiting: Vec<BuildRequest>,
    /// The build thread is running.
    thread: bool,
}

impl Builds {
    /// Drops `workdir`'s waiting build and stops its running one.
    fn forget(&mut self, workdir: &PathBuf) {
        self.waiting.retain(|r| r.workdir != *workdir);
        if let Some((w, stop)) = &self.running
            && w == workdir
        {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct IndexWorker {
    builds: Arc<Mutex<Builds>>,
    query: QueryWorker,
    /// Matches over loaded histories: apart from `query`, so neither replaces the other.
    loaded: QueryWorker,
    /// The saved indexes, once attached.
    disk: Arc<Disk>,
}

impl IndexWorker {
    fn builds(&self) -> MutexGuard<'_, Builds> {
        self.builds.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reads and saves indexes in the store at `dir` from now on.
    pub(super) fn attach_store(&self, dir: PathBuf) {
        self.disk.attach(dir);
    }

    pub(super) fn submit<W: Waker + Clone>(&self, e: IndexEffect, tx: Sender<Msg>, waker: W) {
        match e {
            IndexEffect::Build { workdir, build, key, base } => {
                let start = {
                    let mut b = self.builds();
                    b.forget(&workdir);
                    b.waiting.push(BuildRequest { workdir, build, key, base, stop: Default::default() });
                    let start = !b.thread;
                    b.thread = true;
                    start
                };
                if start {
                    let worker = self.clone();
                    let spawned = std::thread::Builder::new().name("glowfly-index".into()).spawn(move || worker.work(&tx, &waker));
                    if let Err(e) = spawned {
                        tracing::error!("could not start the index thread: {e}");
                        let mut b = self.builds();
                        b.thread = false;
                        b.waiting.clear();
                    }
                }
            }
            IndexEffect::Forget { workdir } => self.builds().forget(&workdir),
            IndexEffect::Query { workdir, request, index, query } => {
                self.query.submit(QueryRequest { workdir, request, haystack: Haystack::Index(index), query }, tx, waker);
            }
            IndexEffect::MatchLoaded { workdir, request, commits, query } => {
                self.loaded.submit(QueryRequest { workdir, request, haystack: Haystack::Loaded(commits.0), query }, tx, waker);
            }
        }
    }

    /// Works through the waiting builds, newest first, until none is left.
    fn work<W: Waker>(&self, tx: &Sender<Msg>, waker: &W) {
        loop {
            let r = {
                let mut b = self.builds();
                match b.waiting.pop() {
                    Some(r) => {
                        b.running = Some((r.workdir.clone(), Arc::clone(&r.stop)));
                        r
                    }
                    None => {
                        b.running = None;
                        b.thread = false;
                        return;
                    }
                }
            };
            let BuildRequest { workdir, build, key, base, stop } = r;
            let mut progress = |p: commit_index::Progress| {
                let msg = SearchMsg::IndexProgress { build, indexed: p.indexed, estimate: p.estimate };
                send(tx, waker, &workdir, msg);
            };
            let stopped = || stop.load(Ordering::Relaxed);
            let result = catch_unwind(AssertUnwindSafe(|| self.disk.build(&workdir, &key, base.as_deref(), &mut progress, &stopped)));
            self.builds().running = None;
            if stopped() {
                continue;
            }
            let (result, save) = match result {
                Ok(Ok(Some(done))) => {
                    tracing::debug!(source = ?done.source, "index ready");
                    (Ok(Arc::new(done.index)), done.save)
                }
                Ok(Ok(None)) => continue,
                Ok(Err(e)) => (Err(e.to_string()), None),
                Err(_) => (Err("Glowfly hit an internal error (index)".to_owned()), None),
            };
            let index = result.as_ref().ok().map(Arc::clone);
            send(tx, waker, &workdir, SearchMsg::IndexBuilt { build, result });
            if let (Some(file), Some(index)) = (save, index) {
                self.disk.save(&file, &index);
            }
        }
    }
}

/// Sends `msg` for the tab of `workdir`.
pub(super) fn send<W: Waker>(tx: &Sender<Msg>, waker: &W, workdir: &std::path::Path, msg: SearchMsg) {
    if tx.send(Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Search(msg)) }).is_ok() {
        waker.wake();
    }
}

/// Runs `e` to the end at once (the test path, `runner::execute`): a build without progress
/// reports, a query or match, and nothing for Forget.
pub(super) fn once(e: IndexEffect) -> Option<Msg> {
    let msg = match e {
        IndexEffect::Build { workdir, build, key, base } => {
            let result = commit_index::build(&workdir, &key, base.as_deref(), &mut |_| {}, &|| false)
                .map_err(|e| e.to_string())
                .and_then(|i| i.map(Arc::new).ok_or_else(|| "stopped".to_owned()));
            SearchMsg::IndexBuilt { build, result }
        }
        IndexEffect::Query { request, index, query, .. } => Haystack::Index(index).answer(&query, request, &|| false)?,
        IndexEffect::MatchLoaded { request, commits, query, .. } => Haystack::Loaded(commits.0).answer(&query, request, &|| false)?,
        IndexEffect::Forget { .. } => return None,
    };
    Some(Msg::Search(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::gitcli::{args, run};
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

    fn repo(name: &str) -> (PathBuf, IndexKey) {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-index-worker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let steps: [&[&str]; 2] = [
            &["init", "-q", "-b", "main"],
            &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "one"],
        ];
        for a in steps {
            let (ok, out) = run(&dir, &args(a));
            assert!(ok, "{out}");
        }
        let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
        (dir, IndexKey { starts: vec![head], shallow: Vec::new() })
    }

    fn build(workdir: &std::path::Path, build: u64, key: &IndexKey) -> IndexEffect {
        IndexEffect::Build { workdir: workdir.to_path_buf(), build, key: key.clone(), base: None }
    }

    /// The build ids `rx` reports as built, after progress reports.
    fn built(rx: &mpsc::Receiver<Msg>) -> Vec<u64> {
        let mut out = Vec::new();
        while let Ok(Msg::Scoped { msg, .. }) = rx.recv_timeout(Duration::from_millis(50)) {
            if let Msg::Search(SearchMsg::IndexBuilt { build, result }) = *msg {
                assert!(result.is_ok(), "{result:?}");
                out.push(build);
            }
        }
        out
    }

    #[test]
    fn a_newer_build_for_the_same_repo_stops_the_running_one() {
        let (dir, key) = repo("newer");
        let (tx, rx) = mpsc::channel();
        let w = IndexWorker::default();
        // Hold the thread off, with a build of this repository running.
        let running = Arc::new(AtomicBool::new(false));
        {
            let mut b = w.builds();
            b.thread = true;
            b.running = Some((dir.clone(), Arc::clone(&running)));
        }
        w.submit(build(&dir, 1, &key), tx.clone(), NoWake);
        assert!(running.load(Ordering::Relaxed), "the running build is told to stop");
        w.submit(build(&dir, 2, &key), tx.clone(), NoWake);
        assert_eq!(w.builds().waiting.iter().map(|r| r.build).collect::<Vec<_>>(), [2], "the waiting one is replaced");
        w.work(&tx, &NoWake);
        assert_eq!(built(&rx), [2]);
        let b = w.builds();
        assert!(!b.thread && b.running.is_none() && b.waiting.is_empty());
        drop(b);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_newest_request_goes_first() {
        let (a, key_a) = repo("first-a");
        let (b, key_b) = repo("first-b");
        let (tx, rx) = mpsc::channel();
        let w = IndexWorker::default();
        w.builds().thread = true;
        w.submit(build(&a, 1, &key_a), tx.clone(), NoWake);
        w.submit(build(&b, 2, &key_b), tx.clone(), NoWake);
        w.work(&tx, &NoWake);
        assert_eq!(built(&rx), [2, 1]);
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn forget_stops_a_build() {
        let (dir, key) = repo("forget");
        let (tx, rx) = mpsc::channel();
        let w = IndexWorker::default();
        let running = Arc::new(AtomicBool::new(false));
        {
            let mut b = w.builds();
            b.thread = true;
            b.running = Some((dir.clone(), Arc::clone(&running)));
        }
        w.submit(build(&dir, 1, &key), tx.clone(), NoWake);
        running.store(false, Ordering::Relaxed);
        w.submit(IndexEffect::Forget { workdir: dir.clone() }, tx.clone(), NoWake);
        assert!(running.load(Ordering::Relaxed), "the running build is stopped");
        assert!(w.builds().waiting.is_empty(), "the waiting build is dropped");
        w.work(&tx, &NoWake);
        assert!(built(&rx).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_worker_reports_before_it_saves() {
        let (dir, key) = repo("disk");
        let store = glowfly_core::paths::temp_dir().join(format!("glowfly-index-worker-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store);
        let (tx, rx) = mpsc::channel();
        let w = IndexWorker::default();
        w.attach_store(store.clone());
        w.builds().thread = true;
        w.submit(build(&dir, 1, &key), tx.clone(), NoWake);
        w.work(&tx, &NoWake);
        assert_eq!(built(&rx), [1]);
        let file = commit_index::Store::new(store.clone()).file_of(&dir).unwrap();
        assert!(file.path.exists(), "saved once work returns");

        // A Forget while the saved index is read: nothing is reported.
        let w = IndexWorker::default();
        w.attach_store(store.clone());
        w.builds().thread = true;
        w.submit(build(&dir, 2, &key), tx.clone(), NoWake);
        w.builds().waiting[0].stop.store(true, Ordering::Relaxed);
        w.work(&tx, &NoWake);
        assert!(built(&rx).is_empty());
        assert!(file.path.exists(), "a stopped read keeps the file");
        let _ = std::fs::remove_dir_all(&store);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! The graph search's path terms: which loaded commits changed a matching path. One worker
//! thread does them, newest request first: a request replaces one still waiting (update ignores
//! its id from then on) and stops one being worked on. A request is reported in chunks of about
//! 150 ms each, newest commits first, so matches fill in from the top.

use super::Waker;
use crate::msg::{Msg, PathsChunk, SearchMsg};
use git2::Oid;
use glowfly_core::path_query::PathTerm;
use glowfly_core::repo;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a chunk may take before it is reported. Tests allow far longer, so a busy machine
/// (the whole suite running in parallel) cannot split a small search into several chunks.
const CHUNK: Duration = if cfg!(test) { Duration::from_secs(30) } else { Duration::from_millis(150) };

pub(super) struct Request {
    pub workdir: PathBuf,
    pub request: u64,
    pub terms: Vec<PathTerm>,
    pub oids: Vec<Oid>,
}

#[derive(Default)]
struct Slot {
    /// The newest request, not started yet.
    next: Option<Request>,
    /// The worker thread is running.
    running: bool,
}

#[derive(Default, Clone)]
pub(super) struct PathSearchWorker(Arc<Mutex<Slot>>);

impl PathSearchWorker {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Asks for `r`, instead of any request still waiting, and stops the one running.
    pub(super) fn submit<W: Waker + Clone>(&self, r: Request, tx: Sender<Msg>, waker: W) {
        let start = {
            let mut s = self.slot();
            s.next = Some(r);
            let start = !s.running;
            s.running = true;
            start
        };
        if !start {
            return;
        }
        let worker = self.clone();
        let (tx2, waker2) = (tx.clone(), waker.clone());
        if let Err(e) = std::thread::Builder::new().name("glowfly-search-paths".into()).spawn(move || worker.work(&tx2, &waker2)) {
            tracing::error!("could not start the path search thread: {e}");
            let mut s = self.slot();
            s.running = false;
            if let Some(r) = s.next.take() {
                drop(s);
                report(&tx, &waker, r.workdir, r.request, Err(format!("could not start a thread: {e}")));
            }
        }
    }

    /// Works through requests until none is waiting.
    fn work<W: Waker>(&self, tx: &Sender<Msg>, waker: &W) {
        loop {
            let Request { workdir, request, terms, oids } = {
                let mut s = self.slot();
                match s.next.take() {
                    Some(r) => r,
                    None => {
                        s.running = false;
                        return;
                    }
                }
            };
            let newer = || self.slot().next.is_some();
            let mut start = 0;
            loop {
                let deadline = Instant::now() + CHUNK;
                let stop = || newer() || Instant::now() >= deadline;
                let rest = &oids[start..];
                match catch_unwind(AssertUnwindSafe(|| repo::commits_touching(&workdir, rest, &terms, &stop))) {
                    Ok(Ok(t)) => {
                        let end = start + t.gone_through;
                        let done = end >= oids.len();
                        if newer() && !done {
                            break;
                        }
                        let chunk = PathsChunk { searched: oids[start..end].to_vec(), touching: t.touching, done };
                        report(tx, waker, workdir.clone(), request, Ok(chunk));
                        if done || t.gone_through == 0 {
                            break;
                        }
                        start = end;
                    }
                    Ok(Err(e)) => {
                        report(tx, waker, workdir.clone(), request, Err(e.to_string()));
                        break;
                    }
                    Err(_) => {
                        report(tx, waker, workdir.clone(), request, Err("Glowfly hit an internal error (search-paths)".into()));
                        break;
                    }
                }
            }
        }
    }
}

fn report<W: Waker>(tx: &Sender<Msg>, waker: &W, workdir: PathBuf, request: u64, result: Result<PathsChunk, String>) {
    let msg = Msg::Search(SearchMsg::PathsSearched { request, result });
    if tx.send(Msg::Scoped { workdir, msg: Box::new(msg) }).is_ok() {
        waker.wake();
    }
}

/// Runs request `r` to the end at once (the test path, `runner::execute`).
pub(super) fn once(r: Request) -> Msg {
    let Request { workdir, request, terms, oids } = r;
    let result = repo::commits_touching(&workdir, &oids, &terms, &|| false)
        .map(|t| PathsChunk { searched: oids, touching: t.touching, done: true })
        .map_err(|e| e.to_string());
    Msg::Search(SearchMsg::PathsSearched { request, result })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::gitcli::{args, run};
    use std::sync::mpsc;

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

    fn git(dir: &std::path::Path, a: &[&str]) {
        let (ok, out) = run(dir, &args(a));
        assert!(ok, "git {a:?} failed: {out}");
    }

    #[test]
    fn a_newer_request_replaces_one_still_waiting() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-path-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "-m", "one"]);
        std::fs::write(dir.join("docs/b.md"), "b\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "-m", "two"]);
        let repo = git2::Repository::open(&dir).unwrap();
        let two = repo.head().unwrap().target().unwrap();
        let one = repo.find_commit(two).unwrap().parent_id(0).unwrap();

        let (tx, rx) = mpsc::channel();
        let w = PathSearchWorker::default();
        // Hold the worker off so both requests are waiting together.
        w.slot().running = true;
        let terms = vec![PathTerm::parse("docs").unwrap()];
        let req = |request, oids| Request { workdir: dir.clone(), request, terms: terms.clone(), oids };
        w.submit(req(1, vec![one]), tx.clone(), NoWake);
        w.submit(req(2, vec![two, one]), tx.clone(), NoWake);
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err(), "the replaced request is dropped silently");
        assert_eq!(w.slot().next.as_ref().map(|r| r.request), Some(2));

        // The worker takes the waiting request, reports it in one chunk, and ends.
        w.work(&tx, &NoWake);
        assert!(!w.slot().running && w.slot().next.is_none());
        let Ok(Msg::Scoped { workdir, msg }) = rx.try_recv() else { panic!("a report") };
        assert_eq!(workdir, dir);
        let Msg::Search(SearchMsg::PathsSearched { request: 2, result: Ok(chunk) }) = *msg else { panic!("request 2's chunk") };
        assert_eq!((chunk.searched, chunk.touching, chunk.done), (vec![two, one], vec![two], true));
        assert!(rx.try_recv().is_err(), "one chunk");
        drop(repo);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

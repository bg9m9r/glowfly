//! Line counts for the graph's CHANGES column. Scrolling asks for the rows coming into view
//! every frame, so one worker thread does them, newest request first: a request replaces one
//! still waiting, and stops one being worked on after its current commit. Whatever is left
//! undone is reported as `RepoMsg::StatsDropped`, so rows still on screen are asked for again.

use super::Waker;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use git2::Oid;
use glowfly_core::repo;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
struct Slot {
    /// The newest request, not started yet.
    next: Option<(PathBuf, Vec<Oid>)>,
    /// The worker thread is running.
    running: bool,
}

#[derive(Default, Clone)]
pub(super) struct StatsWorker(Arc<Mutex<Slot>>);

impl StatsWorker {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Asks for the stats of `oids`, ahead of (and instead of) any request still waiting.
    pub(super) fn submit<W: Waker + Clone>(&self, workdir: PathBuf, oids: Vec<Oid>, tx: Sender<Msg>, waker: W) {
        let (replaced, start) = {
            let mut s = self.slot();
            let replaced = s.next.replace((workdir, oids));
            let start = !s.running;
            s.running = true;
            (replaced, start)
        };
        if let Some((workdir, oids)) = replaced {
            report(&tx, &waker, workdir, Msg::Repo(RepoMsg::StatsDropped(oids)));
        }
        if !start {
            return;
        }
        let worker = self.clone();
        let (tx2, waker2) = (tx.clone(), waker.clone());
        if let Err(e) = std::thread::Builder::new().name("trellis-stats".into()).spawn(move || worker.work(&tx2, &waker2)) {
            tracing::error!("could not start the stats thread: {e}");
            let mut s = self.slot();
            s.running = false;
            if let Some((workdir, oids)) = s.next.take() {
                drop(s);
                report(&tx, &waker, workdir, Msg::Repo(RepoMsg::StatsDropped(oids)));
            }
        }
    }

    /// Works through requests until none is waiting.
    fn work<W: Waker>(&self, tx: &Sender<Msg>, waker: &W) {
        loop {
            let (workdir, oids) = {
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
            // A commit that makes libgit2 panic stays pending rather than being asked for again.
            let (done, gone_through) =
                catch_unwind(AssertUnwindSafe(|| repo::commit_stats_until(&workdir, &oids, &newer))).unwrap_or((Vec::new(), oids.len()));
            if !done.is_empty() {
                report(tx, waker, workdir.clone(), Msg::Repo(RepoMsg::StatsLoaded(done)));
            }
            if gone_through < oids.len() {
                report(tx, waker, workdir, Msg::Repo(RepoMsg::StatsDropped(oids[gone_through..].to_vec())));
            }
        }
    }
}

fn report<W: Waker>(tx: &Sender<Msg>, waker: &W, workdir: PathBuf, msg: Msg) {
    if tx.send(Msg::Scoped { workdir, msg: Box::new(msg) }).is_ok() {
        waker.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

    #[test]
    fn a_newer_request_replaces_one_still_waiting() {
        let (tx, rx) = mpsc::channel();
        let w = StatsWorker::default();
        // Hold the worker off so both requests are waiting together.
        w.slot().running = true;
        let a = Oid::from_bytes(&[1; 20]).unwrap();
        let b = Oid::from_bytes(&[2; 20]).unwrap();
        let dir = glowfly_core::paths::temp_dir().join("trellis-stats-no-repo");
        w.submit(dir.clone(), vec![a], tx.clone(), NoWake);
        w.submit(dir.clone(), vec![b], tx.clone(), NoWake);
        let Ok(Msg::Scoped { workdir, msg }) = rx.recv_timeout(Duration::from_secs(5)) else { panic!("a report") };
        assert_eq!(workdir, dir);
        assert!(matches!(*msg, Msg::Repo(RepoMsg::StatsDropped(ref o)) if *o == [a]), "the older request is dropped");
        assert_eq!(w.slot().next.as_ref().map(|(_, o)| o.clone()), Some(vec![b]));

        // The worker takes the waiting request, and ends once none is left.
        w.work(&tx, &NoWake);
        assert!(!w.slot().running && w.slot().next.is_none());
    }
}

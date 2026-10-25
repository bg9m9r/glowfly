//! Matching a query off the UI thread: against a whole-history index (`IndexEffect::Query`) or a
//! large loaded history (`IndexEffect::MatchLoaded`), each kind on a `QueryWorker` of its own. One
//! worker thread, newest request first: a request replaces one still waiting (update ignores its
//! id from then on) and stops one being matched.

use super::Waker;
use crate::msg::{Msg, SearchMsg};
use glowfly_core::commit_index::CommitIndex;
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::repo::CommitInfo;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

pub(super) struct QueryRequest {
    pub workdir: PathBuf,
    pub request: u64,
    pub haystack: Haystack,
    pub query: CommitQuery,
}

/// What a query is matched against.
pub(super) enum Haystack {
    Index(Arc<CommitIndex>),
    Loaded(Arc<[CommitInfo]>),
}

impl Haystack {
    /// The answer to request `request`, or `None` when `stop` said a newer one is waiting.
    pub(super) fn answer(&self, query: &CommitQuery, request: u64, stop: &dyn Fn() -> bool) -> Option<SearchMsg> {
        Some(match self {
            Haystack::Index(index) => SearchMsg::IndexQueried { request, result: Ok(index.matching(query, stop)?) },
            Haystack::Loaded(commits) => {
                SearchMsg::LoadedMatched { request, result: Ok(query.matching_rows_until(commits, &Default::default(), stop)?) }
            }
        })
    }

    /// The report of request `request` failing with `error`.
    fn failed(&self, request: u64, error: String) -> SearchMsg {
        match self {
            Haystack::Index(_) => SearchMsg::IndexQueried { request, result: Err(error) },
            Haystack::Loaded(_) => SearchMsg::LoadedMatched { request, result: Err(error) },
        }
    }
}

#[derive(Default)]
struct Slot {
    /// The newest request, not started yet.
    next: Option<QueryRequest>,
    /// The worker thread is running.
    running: bool,
}

#[derive(Default, Clone)]
pub(super) struct QueryWorker(Arc<Mutex<Slot>>);

impl QueryWorker {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Asks for `r`, instead of any request still waiting, and stops the one running.
    pub(super) fn submit<W: Waker + Clone>(&self, r: QueryRequest, tx: Sender<Msg>, waker: W) {
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
        if let Err(e) = std::thread::Builder::new().name("glowfly-index-query".into()).spawn(move || worker.work(&tx2, &waker2)) {
            tracing::error!("could not start the index query thread: {e}");
            let mut s = self.slot();
            s.running = false;
            if let Some(r) = s.next.take() {
                drop(s);
                let msg = r.haystack.failed(r.request, format!("could not start a thread: {e}"));
                super::index::send(&tx, &waker, &r.workdir, msg);
            }
        }
    }

    /// Works through requests until none is waiting.
    fn work<W: Waker>(&self, tx: &Sender<Msg>, waker: &W) {
        loop {
            let QueryRequest { workdir, request, haystack, query } = {
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
            let msg = match catch_unwind(AssertUnwindSafe(|| haystack.answer(&query, request, &newer))) {
                Ok(Some(msg)) => msg,
                // A newer request is waiting.
                Ok(None) => continue,
                Err(_) => haystack.failed(request, "Glowfly hit an internal error (index-query)".to_owned()),
            };
            if newer() {
                continue;
            }
            super::index::send(tx, waker, &workdir, msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::commit_index::IndexKey;
    use glowfly_core::repo::CommitInfo;
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

    #[test]
    fn a_newer_query_replaces_a_waiting_one() {
        let info = |n: u8, summary: &str| CommitInfo {
            oid: git2::Oid::from_bytes(&[n; 20]).unwrap(),
            summary: summary.into(),
            body: Default::default(),
            author: glowfly_core::repo::Author::new("A", "a@example.com"),
            time: 0,
            parents: Default::default(),
        };
        let key = IndexKey { starts: Vec::new(), shallow: Vec::new() };
        let index = Arc::new(CommitIndex::from_commits(key, &[info(1, "fix one"), info(2, "other"), info(3, "fix two")]));
        let (tx, rx) = mpsc::channel();
        let w = QueryWorker::default();
        // Hold the worker off so both requests are waiting together.
        w.slot().running = true;
        let req = |request, q: &str| QueryRequest {
            workdir: "r".into(),
            request,
            haystack: Haystack::Index(Arc::clone(&index)),
            query: CommitQuery::parse(q),
        };
        w.submit(req(1, "other"), tx.clone(), NoWake);
        w.submit(req(2, "fix"), tx.clone(), NoWake);
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err(), "the replaced request is dropped silently");
        w.work(&tx, &NoWake);
        assert!(!w.slot().running && w.slot().next.is_none());
        let Ok(Msg::Scoped { workdir, msg }) = rx.try_recv() else { panic!("a report") };
        assert_eq!(workdir, PathBuf::from("r"));
        let Msg::Search(SearchMsg::IndexQueried { request: 2, result: Ok(positions) }) = *msg else { panic!("request 2's answer") };
        assert_eq!(positions, [0, 2]);
        assert!(rx.try_recv().is_err(), "one answer");
    }
}

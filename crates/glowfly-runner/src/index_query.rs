//! Matching a query off the UI thread: against a whole-history index (`IndexEffect::Query`) or a
//! large loaded history (`IndexEffect::MatchLoaded`), each kind on a `QueryWorker` of its own. One
//! worker thread, newest request first: a request replaces one still waiting (update ignores its
//! id from then on) and stops one being matched.

use super::Waker;
use glowfly_core::commit_index::CommitIndex;
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::error::ReadFailed;
use glowfly_core::mailmap::MailmapText;
use glowfly_core::repo::CommitInfo;
use glowfly_model::msg::{Msg, SearchMsg};
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
    /// An index, with the mailmap its stored signatures are shown through.
    Index(Arc<CommitIndex>, MailmapText),
    Loaded(Arc<[CommitInfo]>),
}

impl Haystack {
    /// The answer to request `request`, or `None` when `stop` said a newer one is waiting.
    pub(super) fn answer(&self, query: &CommitQuery, request: u64, stop: &dyn Fn() -> bool) -> Option<SearchMsg> {
        Some(match self {
            Haystack::Index(index, mailmap) => {
                SearchMsg::IndexQueried { request, result: Ok(index.matching(query, &mailmap.open(), stop)?) }
            }
            Haystack::Loaded(commits) => {
                SearchMsg::LoadedMatched { request, result: Ok(query.matching_rows_until(commits, &Default::default(), stop)?) }
            }
        })
    }

    /// The report of request `request` failing with `error`.
    fn failed(&self, request: u64, error: ReadFailed) -> SearchMsg {
        match self {
            Haystack::Index(..) => SearchMsg::IndexQueried { request, result: Err(error) },
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
    pub(super) fn submit(&self, r: QueryRequest, tx: Sender<Msg>, waker: Waker) {
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
                let msg = r.haystack.failed(r.request, ReadFailed::NoThread { said: e.to_string() });
                super::index::send(&tx, &waker, &r.workdir, msg);
            }
        }
    }

    /// Works through requests until none is waiting.
    fn work(&self, tx: &Sender<Msg>, waker: &Waker) {
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
                Err(_) => haystack.failed(request, ReadFailed::Internal { effect: "index-query".into() }),
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
            haystack: Haystack::Index(Arc::clone(&index), MailmapText::default()),
            query: CommitQuery::parse(q),
        };
        w.submit(req(1, "other"), tx.clone(), Waker::none());
        w.submit(req(2, "fix"), tx.clone(), Waker::none());
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err(), "the replaced request is dropped silently");
        w.work(&tx, &Waker::none());
        assert!(!w.slot().running && w.slot().next.is_none());
        let Ok(Msg::Scoped { workdir, msg }) = rx.try_recv() else { panic!("a report") };
        assert_eq!(workdir, PathBuf::from("r"));
        let Msg::Search(SearchMsg::IndexQueried { request: 2, result: Ok(positions) }) = *msg else { panic!("request 2's answer") };
        assert_eq!(positions, [0, 2]);
        assert!(rx.try_recv().is_err(), "one answer");
    }

    #[test]
    fn an_index_query_maps_through_its_mailmap() {
        let info = |n: u8, name: &str, email: &str| CommitInfo {
            oid: git2::Oid::from_bytes(&[n; 20]).unwrap(),
            summary: "work".into(),
            body: Default::default(),
            author: glowfly_core::repo::Author::new(name, email),
            time: 0,
            parents: Default::default(),
        };
        let key = IndexKey { starts: Vec::new(), shallow: Vec::new() };
        let commits =
            [info(1, "Grace Hopper", "grace@example.com"), info(2, "G. Hopper", "ghopper@navy.example"), info(3, "Ada", "ada@example.com")];
        let index = Arc::new(CommitIndex::from_commits(key, &commits));
        let query = CommitQuery::parse("author:grace@example.com");
        let answer = |mailmap: MailmapText| {
            let Some(SearchMsg::IndexQueried { result: Ok(positions), .. }) =
                Haystack::Index(Arc::clone(&index), mailmap).answer(&query, 1, &|| false)
            else {
                panic!("an answer")
            };
            positions
        };
        assert_eq!(answer(MailmapText::default()), [0]);
        assert_eq!(
            answer(MailmapText::from_text(
                "Grace Hopper <grace@example.com> <ghopper@navy.example>
"
            )),
            [0, 1]
        );
    }
}

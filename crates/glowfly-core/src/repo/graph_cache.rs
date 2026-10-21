//! Remembered answers to commit-graph questions asked on every load (ahead/behind counts and
//! whether HEAD contains a branch tip). Each walks history, which on a large repository with
//! many branches adds up to seconds per refresh, while the answer for a pair of commits never
//! changes: commits are named by their content. Shallow repositories aren't cached, as
//! deepening one changes what the same pair can reach. The app keeps one cache for its loads.

use git2::{Oid, Repository};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// Answers kept before the cache starts over.
const MAX_ENTRIES: usize = 8192;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Question {
    AheadBehind(Oid, Oid),
    Contains(Oid, Oid),
}

#[derive(Clone, Copy)]
enum Answer {
    Counts(usize, usize),
    Yes(bool),
}

/// Answers by question, shared by the loads of one app (`repo::load_cached`).
#[derive(Default)]
pub struct GraphCache(Mutex<HashMap<Question, Answer>>);

impl GraphCache {
    fn lock(&self) -> MutexGuard<'_, HashMap<Question, Answer>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ask(&self, repo: &Repository, q: Question, work: impl FnOnce() -> Option<Answer>) -> Option<Answer> {
        let cached = !repo.is_shallow();
        if cached && let Some(a) = self.lock().get(&q).copied() {
            return Some(a);
        }
        let a = work()?;
        if cached {
            let mut map = self.lock();
            if map.len() >= MAX_ENTRIES {
                map.clear();
            }
            map.insert(q, a);
        }
        Some(a)
    }

    /// Commits on `local` but not `upstream`, and the other way round; (0, 0) when unknown.
    pub(super) fn ahead_behind(&self, repo: &Repository, local: Oid, upstream: Oid) -> (usize, usize) {
        let work = || repo.graph_ahead_behind(local, upstream).ok().map(|(a, b)| Answer::Counts(a, b));
        match self.ask(repo, Question::AheadBehind(local, upstream), work) {
            Some(Answer::Counts(a, b)) => (a, b),
            _ => (0, 0),
        }
    }

    /// Whether `tip` is `head` or one of its ancestors; false when unknown.
    pub(super) fn contains(&self, repo: &Repository, head: Oid, tip: Oid) -> bool {
        if head == tip {
            return true;
        }
        let work = || repo.graph_descendant_of(head, tip).ok().map(Answer::Yes);
        matches!(self.ask(repo, Question::Contains(head, tip), work), Some(Answer::Yes(true)))
    }

    #[cfg(test)]
    pub(super) fn has(&self, head: Oid, tip: Oid) -> bool {
        self.lock().contains_key(&Question::Contains(head, tip))
    }
}

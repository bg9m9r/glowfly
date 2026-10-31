//! Remembered answers to commit-graph questions asked on every load (ahead/behind counts and
//! whether HEAD contains a branch tip). Each walks history, which on a large repository with
//! many branches adds up to seconds per refresh, while the answer for a pair of commits never
//! changes: commits are named by their content. Shallow repositories aren't cached, as
//! deepening one changes what the same pair can reach. Past `MAX_ENTRIES` the answers used
//! longest ago go. The recent HEADs are kept too, so a load after HEAD moved on from one of
//! them can start from that one's answers (`branch_merged`). The app keeps one cache for its
//! loads, which also carries the memo of index files its status reads let git record speed-ups
//! in (`index_ext::RecordMemo`).

use git2::{Oid, Repository};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

/// Answers kept; past it, the half used longest ago goes.
const MAX_ENTRIES: usize = 32_768;

/// Recent HEADs kept (of every repository the app loads).
const MAX_HEADS: usize = 32;

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

/// The answers by question, each with when it was last used, and the recent HEADs, newest first.
#[derive(Default)]
struct Answers {
    map: HashMap<Question, (Answer, u64)>,
    clock: u64,
    heads: VecDeque<Oid>,
    /// History walks run for containment, for tests.
    walks: usize,
}

impl Answers {
    fn get(&mut self, q: &Question) -> Option<Answer> {
        self.clock += 1;
        let clock = self.clock;
        self.map.get_mut(q).map(|(a, used)| {
            *used = clock;
            *a
        })
    }

    fn insert(&mut self, q: Question, a: Answer) {
        if self.map.len() >= MAX_ENTRIES && !self.map.contains_key(&q) {
            // Keep the newer half: a load asks each question again, so it keeps its own.
            let mut used: Vec<u64> = self.map.values().map(|(_, u)| *u).collect();
            let (_, &mut keep_from, _) = used.select_nth_unstable(MAX_ENTRIES / 2);
            self.map.retain(|_, (_, u)| *u >= keep_from);
        }
        self.clock += 1;
        self.map.insert(q, (a, self.clock));
    }
}

/// Answers by question, shared by the loads of one app (`repo::load_cached`), and the index
/// record memo its loads and status reads share.
#[derive(Default)]
pub struct GraphCache(Mutex<Answers>, super::RecordMemo);

impl GraphCache {
    /// Which index states a status read already let git record speed-ups in.
    pub fn records(&self) -> &super::RecordMemo {
        &self.1
    }

    fn lock(&self) -> MutexGuard<'_, Answers> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ask(&self, repo: &Repository, q: Question, work: impl FnOnce() -> Option<Answer>) -> Option<Answer> {
        let cached = !repo.is_shallow();
        if cached && let Some(a) = self.lock().get(&q) {
            return Some(a);
        }
        let a = work()?;
        if cached {
            self.lock().insert(q, a);
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
        let work = || {
            self.lock().walks += 1;
            repo.graph_descendant_of(head, tip).ok().map(Answer::Yes)
        };
        matches!(self.ask(repo, Question::Contains(head, tip), work), Some(Answer::Yes(true)))
    }

    /// Whether `head` contains `tip`, when that is known without walking.
    pub(super) fn known_contains(&self, repo: &Repository, head: Oid, tip: Oid) -> Option<bool> {
        if repo.is_shallow() {
            return None;
        }
        match self.lock().get(&Question::Contains(head, tip)) {
            Some(Answer::Yes(yes)) => Some(yes),
            _ => None,
        }
    }

    /// Keeps whether `head` contains `tip`, worked out some other way than `contains`.
    pub(super) fn remember_contains(&self, repo: &Repository, head: Oid, tip: Oid, yes: bool) {
        if !repo.is_shallow() {
            self.lock().insert(Question::Contains(head, tip), Answer::Yes(yes));
        }
    }

    /// The HEADs loaded most recently, newest first.
    pub(super) fn recent_heads(&self) -> Vec<Oid> {
        self.lock().heads.iter().copied().collect()
    }

    /// Notes `head` as the newest HEAD loaded.
    pub(super) fn note_head(&self, head: Oid) {
        let mut a = self.lock();
        a.heads.retain(|h| *h != head);
        a.heads.push_front(head);
        a.heads.truncate(MAX_HEADS);
    }

    #[cfg(test)]
    pub(super) fn has(&self, head: Oid, tip: Oid) -> bool {
        self.lock().map.contains_key(&Question::Contains(head, tip))
    }

    /// How many containment walks ran.
    #[cfg(test)]
    pub(super) fn walks(&self) -> usize {
        self.lock().walks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: u32) -> Question {
        let mut bytes = [0u8; 20];
        bytes[..4].copy_from_slice(&n.to_be_bytes());
        let oid = Oid::from_bytes(&bytes).unwrap();
        Question::Contains(oid, oid)
    }

    #[test]
    fn a_full_cache_drops_the_answers_used_longest_ago() {
        let mut a = Answers::default();
        for n in 0..MAX_ENTRIES as u32 {
            a.insert(q(n), Answer::Yes(true));
        }
        assert!(a.get(&q(0)).is_some(), "used again, so newest");
        a.insert(q(u32::MAX), Answer::Yes(false));
        assert!(a.map.len() <= MAX_ENTRIES / 2 + 1, "{}", a.map.len());
        assert!(a.get(&q(0)).is_some() && a.get(&q(u32::MAX)).is_some());
        assert!(a.get(&q(1)).is_none() && a.get(&q(MAX_ENTRIES as u32 - 1)).is_some());
    }
}

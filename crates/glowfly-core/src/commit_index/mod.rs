//! The whole-history index for the graph's search: every commit the graph's history walk would
//! visit, in the order it visits them, with what search matches in them (the summary, the body
//! flattened as the graph shows it, the author's name and email) and both dates. A large
//! repository's graph loads only its newest commits; the index lets search count and find
//! matches in all of them, and says how far the graph has to load to show one.
//!
//! The index is built off the UI thread (`build.rs`); after a fetch, a commit or a branch change
//! it is updated from the previous index, reading only the commits that one lacks. Text and
//! author terms are matched here exactly as the graph matches loaded commits
//! (`CommitQuery::text_matches`, `CommitQuery::author_matches`); path terms are not indexed.
//!
//! Memory: about 110 bytes per commit plus its summary and capped body, roughly 300 to 400 bytes
//! per commit on histories with long messages. Texts live in blocks of about 1 MiB that an
//! updated index shares with the one it was built from.

use crate::commit_query::CommitQuery;
use crate::matching::contains_lower;
use crate::repo::{Author, CommitInfo, Snapshot};
use git2::Oid;
use std::collections::HashMap;
use std::sync::Arc;

mod build;
pub mod estimate;

pub use build::{Progress, build};

#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod tests;

/// The most commits one index holds; a longer history is indexed up to here (`complete` false).
pub const MAX_INDEXED: usize = 2_000_000;

/// The size a text block grows to before the next one starts.
const BLOCK: usize = 1 << 20;

/// How often (in entries) a long match checks whether it should stop.
const MATCH_CHECK: usize = 8192;

/// What decides which commits an index holds and in which order: the walk's starting commits in
/// the order pushed (`Snapshot::starts`) and the shallow clone's boundary (`Snapshot::shallow_roots`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexKey {
    pub starts: Vec<Oid>,
    pub shallow: Vec<Oid>,
}

impl IndexKey {
    /// The key of the history `snap` was walked from.
    pub fn of(snap: &Snapshot) -> IndexKey {
        IndexKey { starts: snap.starts.clone(), shallow: snap.shallow_roots.clone() }
    }

    /// Whether this is the key of the history `snap` was walked from.
    pub fn covers(&self, snap: &Snapshot) -> bool {
        self.starts == snap.starts && self.shallow == snap.shallow_roots
    }
}

/// One indexed commit.
#[derive(Clone, Copy)]
struct Entry {
    oid: Oid,
    /// Committer time (seconds): what orders the walk.
    time: i64,
    author_time: i64,
    /// Index into `people`.
    person: u32,
    /// The text block holding the summary then the body, and where in it they start.
    block: u32,
    at: u32,
    summary_len: u32,
    body_len: u32,
    /// Where its walked parents are in `parents`, and how many.
    parents_at: u32,
    parents_len: u32,
}

/// The index of one tab's history (see the module docs). Positions are walk order: the graph
/// shows the commit at position `p` once it loads at least `p + 1` commits.
pub struct CommitIndex {
    key: IndexKey,
    /// False when the walk stopped at the cap (`MAX_INDEXED`) with commits left.
    complete: bool,
    entries: Vec<Entry>,
    by_oid: HashMap<Oid, u32>,
    /// Every entry's walked parents, one after another.
    parents: Vec<Oid>,
    texts: Vec<Arc<str>>,
    /// Distinct (name, email) authors, looked up without allocating (`Author: Borrow<str>`).
    people: Vec<Author>,
    person_of: HashMap<Author, u32>,
}

impl std::fmt::Debug for CommitIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitIndex").field("commits", &self.entries.len()).field("complete", &self.complete).finish()
    }
}

impl CommitIndex {
    /// How many commits it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether it holds the whole history (the walk wasn't stopped by the cap).
    pub fn complete(&self) -> bool {
        self.complete
    }

    /// The history it was built for.
    pub fn key(&self) -> &IndexKey {
        &self.key
    }

    /// The commit at walk position `pos`.
    pub fn oid_at(&self, pos: usize) -> Option<Oid> {
        self.entries.get(pos).map(|e| e.oid)
    }

    /// The walk position of commit `oid`.
    pub fn position(&self, oid: Oid) -> Option<u32> {
        self.by_oid.get(&oid).copied()
    }

    /// The summary of the commit at `pos`.
    pub fn summary_at(&self, pos: usize) -> Option<&str> {
        self.entries.get(pos).map(|e| self.texts_of(e).0)
    }

    /// The flattened body of the commit at `pos`.
    pub fn body_at(&self, pos: usize) -> Option<&str> {
        self.entries.get(pos).map(|e| self.texts_of(e).1)
    }

    /// The author (name, email) of the commit at `pos`.
    pub fn person_at(&self, pos: usize) -> Option<(&str, &str)> {
        self.entries.get(pos).map(|e| {
            let a = &self.people[e.person as usize];
            (a.name(), a.email())
        })
    }

    /// The (committer, author) times of the commit at `pos`, in seconds.
    pub fn times_at(&self, pos: usize) -> Option<(i64, i64)> {
        self.entries.get(pos).map(|e| (e.time, e.author_time))
    }

    fn texts_of(&self, e: &Entry) -> (&str, &str) {
        // A block is kept only once something was written to it.
        if e.summary_len == 0 && e.body_len == 0 {
            return ("", "");
        }
        let block = &self.texts[e.block as usize];
        let at = e.at as usize;
        let mid = at + e.summary_len as usize;
        (&block[at..mid], &block[mid..mid + e.body_len as usize])
    }

    /// The walk positions of the commits matching `q`'s text and author terms, ascending; empty
    /// for an empty query. Path terms are not indexed and must not be asked for. `None` when
    /// `stop` said to stop (it is asked every few thousand commits).
    pub fn matching(&self, q: &CommitQuery, stop: &dyn Fn() -> bool) -> Option<Vec<u32>> {
        debug_assert!(!q.has_paths(), "path terms aren't indexed");
        if q.is_empty() {
            return Some(Vec::new());
        }
        // Each author is tested once, not once per commit.
        let author_ok: Vec<bool> = self.people.iter().map(|a| q.author_matches(a.name(), a.email())).collect();
        let name_text: Vec<bool> = self.people.iter().map(|a| contains_lower(a.name(), q.text())).collect();
        let text = !q.text().is_empty();
        let mut out = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if i.is_multiple_of(MATCH_CHECK) && stop() {
                return None;
            }
            let p = e.person as usize;
            if !author_ok[p] {
                continue;
            }
            let hit = !text || name_text[p] || {
                let (summary, body) = self.texts_of(e);
                q.text_matches(&e.oid, summary, body, "")
            };
            if hit {
                out.push(i as u32);
            }
        }
        Some(out)
    }

    /// An index of `commits` taken as given in walk order (the whole history: `complete`). For
    /// tests and fixtures; real indexes come from [`build`].
    pub fn from_commits(key: IndexKey, commits: &[CommitInfo]) -> CommitIndex {
        let mut b = Builder::new(key, None);
        for c in commits {
            let text = Texts { summary: &c.summary, body: &c.body };
            b.push_new(c.oid, (c.time, c.time), (c.author.name(), c.author.email()), text, &c.parents);
        }
        b.finish(true)
    }

    /// This index as if its walk had stopped at the cap (`complete` false). For tests and
    /// fixtures.
    pub fn marked_incomplete(mut self) -> CommitIndex {
        self.complete = false;
        self
    }
}

/// A commit's texts as stored.
struct Texts<'a> {
    summary: &'a str,
    body: &'a str,
}

/// An index being filled in walk order, sharing the text blocks and authors of the index it
/// updates (if any).
struct Builder {
    ix: CommitIndex,
    /// The text block being filled; it becomes `texts[block]` when full or at the end.
    current: String,
    /// Scratch space for an author's lookup key.
    key: String,
}

impl Builder {
    fn new(key: IndexKey, base: Option<&CommitIndex>) -> Builder {
        let (texts, people, person_of) = match base {
            Some(b) => (b.texts.clone(), b.people.clone(), b.person_of.clone()),
            None => Default::default(),
        };
        let ix = CommitIndex {
            key,
            complete: false,
            entries: Vec::new(),
            by_oid: HashMap::new(),
            parents: Vec::new(),
            texts,
            people,
            person_of,
        };
        Builder { ix, current: String::new(), key: String::new() }
    }

    fn len(&self) -> usize {
        self.ix.entries.len()
    }

    fn person(&mut self, name: &str, email: &str) -> u32 {
        Author::key_into(&mut self.key, name, email);
        if let Some(&p) = self.ix.person_of.get(self.key.as_str()) {
            return p;
        }
        let p = self.ix.people.len() as u32;
        let a = Author::new(name, email);
        self.ix.people.push(a.clone());
        self.ix.person_of.insert(a, p);
        p
    }

    fn push_parents(&mut self, parents: &[Oid]) -> (u32, u32) {
        let at = self.ix.parents.len() as u32;
        self.ix.parents.extend_from_slice(parents);
        (at, parents.len() as u32)
    }

    fn push_entry(&mut self, e: Entry) {
        self.ix.by_oid.insert(e.oid, self.ix.entries.len() as u32);
        self.ix.entries.push(e);
    }

    /// A commit read from the repository: its texts go into the current block.
    fn push_new(&mut self, oid: Oid, (time, author_time): (i64, i64), (name, email): (&str, &str), t: Texts, parents: &[Oid]) {
        let need = t.summary.len() + t.body.len();
        if !self.current.is_empty() && self.current.len() + need > BLOCK {
            let full = std::mem::take(&mut self.current);
            self.ix.texts.push(Arc::from(full));
        }
        let at = self.current.len() as u32;
        self.current.push_str(t.summary);
        self.current.push_str(t.body);
        let person = self.person(name, email);
        let (parents_at, parents_len) = self.push_parents(parents);
        let block = self.ix.texts.len() as u32;
        let e = Entry {
            oid,
            time,
            author_time,
            person,
            block,
            at,
            summary_len: t.summary.len() as u32,
            body_len: t.body.len() as u32,
            parents_at,
            parents_len,
        };
        self.push_entry(e);
    }

    /// A commit the base index holds (`e` is its entry there): texts and author are shared.
    fn push_copied(&mut self, e: &Entry, parents: &[Oid]) {
        let (parents_at, parents_len) = self.push_parents(parents);
        self.push_entry(Entry { parents_at, parents_len, ..*e });
    }

    fn finish(mut self, complete: bool) -> CommitIndex {
        if !self.current.is_empty() {
            self.ix.texts.push(Arc::from(self.current));
        }
        self.ix.complete = complete;
        self.ix
    }
}

impl CommitIndex {
    /// The entry of `oid` and its walked parents, when this index holds it.
    fn walked(&self, oid: Oid) -> Option<(&Entry, &[Oid])> {
        let e = &self.entries[*self.by_oid.get(&oid)? as usize];
        let at = e.parents_at as usize;
        Some((e, &self.parents[at..at + e.parents_len as usize]))
    }

    /// The committer time of `oid`, when this index holds it.
    fn time_of(&self, oid: Oid) -> Option<i64> {
        self.by_oid.get(&oid).map(|&p| self.entries[p as usize].time)
    }
}

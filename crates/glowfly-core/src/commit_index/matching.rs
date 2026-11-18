//! Matching a query against the whole-history index: the text and author terms, applied to
//! every indexed commit the way the graph applies them to loaded ones. The index holds
//! signatures as written; the mailmap maps each distinct person once per query.

use super::CommitIndex;
use crate::commit_query::CommitQuery;
use crate::mailmap::Mailmap;
use crate::matching::contains_lower;

/// How often (in entries) a long match checks whether it should stop.
const MATCH_CHECK: usize = 8192;

impl CommitIndex {
    /// The walk positions of the commits matching `q`'s text and author terms, ascending; empty
    /// for an empty query. Path terms are not indexed and must not be asked for. Names and
    /// emails are matched as `map` shows them. `None` when `stop` said to stop (it is asked
    /// every few thousand commits).
    pub fn matching(&self, q: &CommitQuery, map: &Mailmap, stop: &dyn Fn() -> bool) -> Option<Vec<u32>> {
        debug_assert!(!q.has_paths(), "path terms aren't indexed");
        if q.is_empty() {
            return Some(Vec::new());
        }
        // Each author is mapped and tested once, not once per commit.
        let shown: Vec<Option<(String, String)>> =
            if map.is_none() { Vec::new() } else { self.people.iter().map(|a| map.resolve(a.name(), a.email())).collect() };
        let person = |p: usize| match shown.get(p) {
            Some(Some((name, email))) => (name.as_str(), email.as_str()),
            _ => (self.people[p].name(), self.people[p].email()),
        };
        let author_ok: Vec<bool> = (0..self.people.len()).map(|p| q.author_matches(person(p).0, person(p).1)).collect();
        let name_text: Vec<bool> = (0..self.people.len()).map(|p| contains_lower(person(p).0, q.text())).collect();
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
}

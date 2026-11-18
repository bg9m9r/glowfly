//! A search's matched text on a matching graph row: where the query's free text falls in the
//! summary, the body preview and the author's name, the matched start of the short SHA, and an
//! `author:` term's match in the name (the whole name when the term matched only the email).
//! Each run is drawn in the row's full ink on the find fill (widgets/marked_text.rs).

pub(super) use crate::widgets::marked_text::{Mark, Runs, paint_cut_marked};
use crate::widgets::marked_text::{merge, runs};
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::matching::{contains_lower, hex_starts_with};
use glowfly_core::repo::CommitInfo;

/// Where the query matched one commit's shown text.
#[derive(Default)]
pub(super) struct RowRuns {
    pub summary: Runs,
    pub body: Runs,
    pub author: Runs,
    pub sha: Runs,
}

impl RowRuns {
    /// The runs of `q` in commit `c` (whose body preview, if shown, is `body`).
    pub fn of(q: &CommitQuery, c: &CommitInfo, body: Option<&str>) -> RowRuns {
        let text = q.text();
        let name = c.author.name();
        let mut author = runs(name, text);
        for a in q.authors() {
            let found = runs(name, a);
            // A term that matched only the address (a picked email, say) marks the whole name.
            if found.is_empty() && contains_lower(c.author.email(), a) {
                author.push(0..name.chars().count());
            }
            author.extend(found);
        }
        let sha = match text.len() {
            n @ 1..=40 if hex_starts_with(c.oid.as_bytes(), text) => std::iter::once(0..n.min(7)).collect(),
            _ => Vec::new(),
        };
        RowRuns { summary: runs(&c.summary, text), body: body.map_or_else(Vec::new, |b| runs(b, text)), author: merge(author), sha }
    }
}

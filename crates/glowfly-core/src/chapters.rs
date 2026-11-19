//! The Chapters view's grouping: the first-parent history of HEAD read as a list of things that
//! happened. A merge is one chapter (with the commits it brought in), a squash landing is one,
//! and neighbouring commits by the same author close in time form a run. A pure computation
//! over the loaded commits (no git reads): `group` is re-run when the history changes.
//!
//! Every rule looks only at a commit and its older neighbour, so loading more history can only
//! grow the oldest chapter (and a merge whose merged-in history reaches below what is loaded).
//! `words` holds the text rules (squash numbers, closing lines, merge titles, authors) and
//! `release` the release-tag heuristic.

mod release;
mod words;

use crate::repo::{Author, CommitInfo, Snapshot};
use git2::Oid;
use std::collections::HashMap;

pub use release::is_release_tag;

/// The longest gap between two commits of one run, in seconds.
pub const GAP_SECS: i64 = 8 * 3600;
/// How many authors a chapter names; the rest are counted.
pub const AUTHORS_KEPT: usize = 3;

/// What `group` reads: the loaded commits and the few things around them.
pub struct History<'a> {
    pub commits: &'a [CommitInfo],
    pub row_of: &'a HashMap<Oid, usize>,
    pub head: Option<Oid>,
    pub tags: &'a [(String, Oid)],
    pub remotes: Vec<&'a str>,
}

impl<'a> History<'a> {
    pub fn of(snap: &'a Snapshot) -> Self {
        History {
            commits: &snap.commits,
            row_of: &snap.row_of,
            head: snap.head_oid,
            tags: &snap.tags,
            remotes: snap.remotes.iter().map(|(name, _)| name.as_str()).collect(),
        }
    }
}

/// What makes a chapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A merge commit on the line, with what it brought in.
    Merge,
    /// A single-parent commit whose subject ends in `(#n)`: a squash landing.
    Squash,
    /// One or more neighbouring commits by one author, close in time.
    Run,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    /// Its newest first-parent commit: the chapter's stable identity.
    pub id: Oid,
    pub kind: Kind,
    pub title: String,
    /// Rows (indexes into the commits) of its first-parent commits, newest first; exactly one
    /// for a merge and a squash landing.
    pub line: Vec<u32>,
    /// Rows of the commits a merge brought in, ascending (newest first); empty otherwise.
    pub side: Vec<u32>,
    /// The merged-in history goes on below the loaded commits.
    pub partial: bool,
    /// Distinct authors in order of first appearance (line newest first, then side), the first
    /// `AUTHORS_KEPT` kept.
    pub authors: Vec<Author>,
    /// The authors beyond `AUTHORS_KEPT`.
    pub more_authors: u32,
    /// The newest and the oldest committer time over line and side.
    pub newest: i64,
    pub oldest: i64,
    /// Release-looking tags on the newest commit, sorted; empty means no heading.
    pub releases: Vec<String>,
}

impl Chapter {
    /// How many commits it covers.
    pub fn count(&self) -> usize {
        self.line.len() + self.side.len()
    }

    /// The commits shown inside the chapter when it is expanded: what a merge brought in, or a
    /// run's commits; none for a chapter of one.
    pub fn inside(&self) -> &[u32] {
        match self.kind {
            Kind::Merge => &self.side,
            _ if self.line.len() > 1 => &self.line,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chapters {
    /// Newest first.
    pub list: Vec<Chapter>,
    /// The first-parent line goes on below the loaded commits.
    pub open_end: bool,
}

impl Chapters {
    pub fn index_of(&self, id: Oid) -> Option<usize> {
        self.list.iter().position(|c| c.id == id)
    }
}

/// What `group` knows about one commit of the line.
struct Mark {
    kind: Kind,
    closes: bool,
    release: bool,
}

/// Groups HEAD's first-parent history into chapters.
pub fn group(h: &History) -> Chapters {
    let line = first_parent_line(h);
    let Some(&last) = line.last() else {
        // HEAD's own commit is not among the loaded ones (other tips hold the newest commits):
        // its line lies entirely below them. No HEAD at all (an unborn branch) has none.
        return Chapters { list: Vec::new(), open_end: h.head.is_some() };
    };
    let open_end = h.commits.get(last).and_then(|c| c.parents.first()).is_some_and(|p| !h.row_of.contains_key(p));
    let releases = release_map(h);
    let marks: Vec<Mark> = line
        .iter()
        .map(|&row| {
            let c = &h.commits[row];
            let kind = if c.parents.len() > 1 {
                Kind::Merge
            } else if words::squash_number(&c.summary).is_some() {
                Kind::Squash
            } else {
                Kind::Run
            };
            Mark { kind, closes: words::closes_issue(&c.body), release: releases.contains_key(&row) }
        })
        .collect();
    let mut list = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let mut j = i + 1;
        if marks[i].kind == Kind::Run {
            while j < line.len()
                && marks[j].kind == Kind::Run
                && !marks[j].closes
                && !marks[j].release
                && words::same_author(&h.commits[line[j - 1]].author, &h.commits[line[j]].author)
                && (h.commits[line[j - 1]].time - h.commits[line[j]].time).abs() <= GAP_SECS
            {
                j += 1;
            }
        }
        list.push(Chapter {
            id: h.commits[line[i]].oid,
            kind: marks[i].kind,
            title: String::new(),
            line: line[i..j].iter().map(|&r| r as u32).collect(),
            side: Vec::new(),
            partial: false,
            authors: Vec::new(),
            more_authors: 0,
            newest: 0,
            oldest: 0,
            releases: Vec::new(),
        });
        i = j;
    }
    claim_sides(h, &mut list);
    let mut at = 0;
    for ch in &mut list {
        summarise(h, ch, marks[at].closes, &releases);
        at += ch.line.len();
    }
    Chapters { list, open_end }
}

/// The rows of HEAD, its first parent and so on while they are loaded.
fn first_parent_line(h: &History) -> Vec<usize> {
    let mut line = Vec::new();
    let mut at = h.head.and_then(|o| h.row_of.get(&o).copied());
    while let Some(row) = at {
        line.push(row);
        // A loaded history is acyclic; the guard keeps a damaged one from looping.
        if line.len() > h.commits.len() {
            break;
        }
        at = h.commits.get(row).and_then(|c| c.parents.first()).and_then(|p| h.row_of.get(p).copied());
    }
    line
}

/// The release-looking tags by the row of the commit they point at, names sorted.
fn release_map(h: &History) -> HashMap<usize, Vec<String>> {
    let mut map: HashMap<usize, Vec<String>> = HashMap::new();
    for (name, oid) in h.tags {
        if let (true, Some(&row)) = (release::is_release_tag(name), h.row_of.get(oid)) {
            map.entry(row).or_default().push(name.clone());
        }
    }
    for names in map.values_mut() {
        names.sort();
        names.dedup();
    }
    map
}

/// Gives each merge the loaded commits reachable from its other parents but not from its first:
/// the oldest merge claims first, so a commit lands in the oldest line commit that reaches it.
/// (With skewed clocks a commit can move to an older merge after a longer load.)
fn claim_sides(h: &History, list: &mut [Chapter]) {
    let mut visited = vec![false; h.commits.len()];
    for ch in list.iter() {
        for &row in &ch.line {
            visited[row as usize] = true;
        }
    }
    for ch in list.iter_mut().rev().filter(|c| c.kind == Kind::Merge) {
        let mut stack: Vec<Oid> = h.commits[ch.line[0] as usize].parents.iter().skip(1).copied().collect();
        while let Some(oid) = stack.pop() {
            let Some(&row) = h.row_of.get(&oid) else {
                ch.partial = true;
                continue;
            };
            if visited[row] {
                continue;
            }
            visited[row] = true;
            ch.side.push(row as u32);
            stack.extend(h.commits[row].parents.iter().copied());
        }
        ch.side.sort_unstable();
    }
}

/// Fills the title, authors, times and release tags of `ch`; `closes` says whether its newest
/// commit has a closing line.
fn summarise(h: &History, ch: &mut Chapter, closes: bool, releases: &HashMap<usize, Vec<String>>) {
    let first = ch.line[0] as usize;
    let rows = || ch.line.iter().chain(&ch.side).map(|&r| &h.commits[r as usize]);
    let mut authors: Vec<&Author> = Vec::new();
    for c in rows() {
        if !authors.iter().any(|a| words::same_author(a, &c.author)) {
            authors.push(&c.author);
        }
    }
    let (newest, oldest) = rows().fold((i64::MIN, i64::MAX), |(hi, lo), c| (hi.max(c.time), lo.min(c.time)));
    let more_authors = authors.len().saturating_sub(AUTHORS_KEPT) as u32;
    let kept: Vec<Author> = authors.into_iter().take(AUTHORS_KEPT).cloned().collect();
    let newest_commit = &h.commits[first];
    let title = match ch.kind {
        Kind::Merge => words::merge_title(&newest_commit.summary, &h.remotes),
        Kind::Squash => newest_commit.summary.trim().to_owned(),
        Kind::Run if closes => newest_commit.summary.trim().to_owned(),
        Kind::Run => ch.line.last().map_or_else(String::new, |&r| h.commits[r as usize].summary.trim().to_owned()),
    };
    ch.title = if title.is_empty() { ch.id.to_string().chars().take(7).collect() } else { title };
    ch.more_authors = more_authors;
    ch.authors = kept;
    ch.newest = newest;
    ch.oldest = oldest;
    ch.releases = releases.get(&first).cloned().unwrap_or_default();
}

#[cfg(test)]
mod tests;

//! Commit search queries for the graph: free text plus `author:` and `path:` terms, the loaded
//! commits that match, the history's distinct authors (for the author picker) and rewriting a
//! query's author term.
//!
//! Syntax: whitespace-separated words. A word starting with `author:` (any case) is an author
//! term, and one starting with `path:` or `file:` a path term ([`PathTerm`]); a term's value may
//! be quoted to hold spaces (`author:"Ada Lovelace"`, `path:"My Docs/"`), and a quote left open
//! runs to the end of the query. Every other word is free text, matched as one phrase (words
//! joined by single spaces) against the summary, body and author name, or as a prefix of the
//! commit id. An author value that looks like a full email address (as the author picker writes
//! it) must equal the commit's email; any other value is found in the author name or email.
//! Several author terms are alternatives, and so are several path terms; text, authors and
//! paths must all match. Path terms need each commit's changed files, so [`CommitQuery::matches`]
//! leaves them out and [`CommitQuery::matching_rows`] takes the commits found touching them.

use crate::matching::{contains_lower, hex_starts_with};
use crate::path_query::PathTerm;
use crate::repo::CommitInfo;
use git2::Oid;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

const AUTHOR: &str = "author:";
const PATH_PREFIXES: [&str; 2] = ["path:", "file:"];

/// A parsed search query. The text parts are lowercase, so matching never allocates for ASCII
/// text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitQuery {
    /// The free text: the words that aren't author or path terms, joined by single spaces.
    text: String,
    /// The author terms' values (never empty).
    authors: Vec<String>,
    /// The path terms, without repeats, in the order written.
    paths: Vec<PathTerm>,
}

/// One word of a query, as written.
enum Token<'a> {
    Word(&'a str),
    /// An author term's value, without the prefix and quotes.
    Author(&'a str),
    /// A path term's value, without the prefix and quotes, and the whole term as written.
    Path {
        value: &'a str,
        raw: &'a str,
    },
}

#[derive(Clone, Copy)]
enum Kind {
    Author,
    Path,
}

/// The kind of term `rest` starts with (its prefix in any case), and what follows the prefix.
fn prefixed(rest: &str) -> Option<(Kind, &str)> {
    let starts = |p: &str| rest.get(..p.len()).is_some_and(|s| s.eq_ignore_ascii_case(p));
    if starts(AUTHOR) {
        return Some((Kind::Author, &rest[AUTHOR.len()..]));
    }
    PATH_PREFIXES.iter().find(|p| starts(p)).map(|p| (Kind::Path, &rest[p.len()..]))
}

/// Splits `q` into words and author and path terms.
fn tokens(q: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = q;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return out;
        }
        if let Some((kind, after)) = prefixed(rest) {
            let (value, tail) = match after.strip_prefix('"') {
                Some(inner) => match inner.find('"') {
                    Some(end) => (&inner[..end], &inner[end + 1..]),
                    None => (inner, ""),
                },
                None => after.split_at(after.find(char::is_whitespace).unwrap_or(after.len())),
            };
            let raw = &rest[..rest.len() - tail.len()];
            out.push(match kind {
                Kind::Author => Token::Author(value),
                Kind::Path => Token::Path { value, raw },
            });
            rest = tail;
        } else {
            let (word, tail) = rest.split_at(rest.find(char::is_whitespace).unwrap_or(rest.len()));
            out.push(Token::Word(word));
            rest = tail;
        }
    }
}

impl CommitQuery {
    /// Parses a query typed in the search field.
    pub fn parse(q: &str) -> CommitQuery {
        let mut words = Vec::new();
        let mut authors = Vec::new();
        let mut paths: Vec<PathTerm> = Vec::new();
        for t in tokens(q) {
            match t {
                Token::Word(w) => words.push(w),
                Token::Author(a) => {
                    let a = a.trim().to_lowercase();
                    if !a.is_empty() {
                        authors.push(a);
                    }
                }
                Token::Path { value, .. } => {
                    if let Some(p) = PathTerm::parse(value)
                        && !paths.contains(&p)
                    {
                        paths.push(p);
                    }
                }
            }
        }
        CommitQuery { text: words.join(" ").to_lowercase(), authors, paths }
    }

    /// Whether there is nothing to search for (no free text, author value or path term).
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.authors.is_empty() && self.paths.is_empty()
    }

    /// The author terms' values, lowercase.
    pub fn authors(&self) -> &[String] {
        &self.authors
    }

    /// The path terms, in the order written.
    pub fn paths(&self) -> &[PathTerm] {
        &self.paths
    }

    /// Whether the query has a path term, so its matches need the commits' changed files.
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Whether commit `c` matches the text and author terms: the free text (if any) is in its
    /// summary, body or author name, or starts its id, and one of the author values (if any)
    /// matches its author (see [`author_matches`]). Path terms are checked by
    /// [`matching_rows`](Self::matching_rows).
    pub fn matches(&self, c: &CommitInfo) -> bool {
        let q = self.text.as_str();
        let text = q.is_empty()
            || contains_lower(&c.summary, q)
            || contains_lower(&c.body, q)
            || contains_lower(&c.author, q)
            || hex_starts_with(c.oid.as_bytes(), q);
        text && (self.authors.is_empty() || self.authors.iter().any(|a| author_matches(a, c)))
    }

    /// Indexes of the matching commits, ascending; empty for an empty query. With path terms a
    /// commit must also be in `touching` (the commits found to change a matching path); without
    /// them `touching` is ignored.
    pub fn matching_rows(&self, commits: &[CommitInfo], touching: &HashSet<Oid>) -> Vec<usize> {
        if self.is_empty() {
            return Vec::new();
        }
        let paths = |c: &CommitInfo| self.paths.is_empty() || touching.contains(&c.oid);
        commits.iter().enumerate().filter(|(_, c)| self.matches(c) && paths(c)).map(|(i, _)| i).collect()
    }
}

/// Whether `value` (lowercase) is a whole email address: one `@` with text on both sides, a
/// domain holding a dot that isn't its first or last character, and no whitespace.
fn is_full_address(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !value.contains(char::is_whitespace)
}

/// Whether author value `value` (lowercase) matches commit `c`: a full address must equal its
/// email ignoring case, so picking `ann@example.com` leaves out `joann@example.com`; any other
/// value need only be in its author name or email.
fn author_matches(value: &str, c: &CommitInfo) -> bool {
    if is_full_address(value) {
        c.email.chars().flat_map(char::to_lowercase).eq(value.chars())
    } else {
        contains_lower(&c.author, value) || contains_lower(&c.email, value)
    }
}

/// One author of the loaded history.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthorCount {
    /// The name on their newest commit.
    pub name: String,
    /// The email as spelled on their newest commit.
    pub email: String,
    pub commits: usize,
}

/// The distinct authors of `commits` (newest first), grouped by email ignoring case, most
/// commits first, then by name.
pub fn authors(commits: &[CommitInfo]) -> Vec<AuthorCount> {
    let mut out: Vec<AuthorCount> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for c in commits {
        let key = c.email.to_lowercase();
        match index.get(&key) {
            Some(&i) => out[i].commits += 1,
            None => {
                index.insert(key, out.len());
                out.push(AuthorCount { name: c.author.clone(), email: c.email.clone(), commits: 1 });
            }
        }
    }
    out.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// `query` with its author terms replaced by one for `author` (placed first), or removed when
/// `author` is `None`. The other words and the path terms keep their order and spelling.
pub fn with_author(query: &str, author: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = author.map(str::trim).filter(|a| !a.is_empty()) {
        let needs_quotes = a.contains(char::is_whitespace) || a.starts_with('"');
        parts.push(if needs_quotes { format!("{AUTHOR}\"{}\"", a.replace('"', "")) } else { format!("{AUTHOR}{a}") });
    }
    for t in tokens(query) {
        match t {
            Token::Word(w) | Token::Path { raw: w, .. } => parts.push(w.to_owned()),
            Token::Author(_) => {}
        }
    }
    parts.join(" ")
}

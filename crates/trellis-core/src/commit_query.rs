//! Commit search queries for the graph: free text plus `author:` terms, the loaded commits that
//! match, the history's distinct authors (for the author picker) and rewriting a query's author
//! term.
//!
//! Syntax: whitespace-separated words. A word starting with `author:` (any case) is an author
//! term; its value may be quoted to hold spaces (`author:"Ada Lovelace"`), and a quote left open
//! runs to the end of the query. Every other word is free text, matched as one phrase (words
//! joined by single spaces) against the summary, body and author name, or as a prefix of the
//! commit id. An author value that looks like a full email address (as the author picker writes
//! it) must equal the commit's email; any other value is found in the author name or email.
//! Several author terms are alternatives; text and authors must both match.

use crate::matching::{contains_lower, hex_starts_with};
use crate::repo::CommitInfo;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

const AUTHOR: &str = "author:";

/// A parsed search query. Both parts are lowercase, so matching never allocates for ASCII text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitQuery {
    /// The free text: the words that aren't author terms, joined by single spaces.
    text: String,
    /// The author terms' values (never empty).
    authors: Vec<String>,
}

/// One word of a query, as written.
enum Token<'a> {
    Word(&'a str),
    /// An author term's value, without the prefix and quotes.
    Author(&'a str),
}

/// Splits `q` into words and author terms.
fn tokens(q: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = q;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return out;
        }
        let is_author = rest.get(..AUTHOR.len()).is_some_and(|p| p.eq_ignore_ascii_case(AUTHOR));
        if is_author {
            let after = &rest[AUTHOR.len()..];
            let (value, tail) = match after.strip_prefix('"') {
                Some(inner) => match inner.find('"') {
                    Some(end) => (&inner[..end], &inner[end + 1..]),
                    None => (inner, ""),
                },
                None => after.split_at(after.find(char::is_whitespace).unwrap_or(after.len())),
            };
            out.push(Token::Author(value));
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
        for t in tokens(q) {
            match t {
                Token::Word(w) => words.push(w),
                Token::Author(a) => {
                    let a = a.trim().to_lowercase();
                    if !a.is_empty() {
                        authors.push(a);
                    }
                }
            }
        }
        CommitQuery { text: words.join(" ").to_lowercase(), authors }
    }

    /// Whether there is nothing to search for (no free text and no author value).
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.authors.is_empty()
    }

    /// The author terms' values, lowercase.
    pub fn authors(&self) -> &[String] {
        &self.authors
    }

    /// Whether commit `c` matches: the free text (if any) is in its summary, body or author
    /// name, or starts its id, and one of the author values (if any) matches its author (see
    /// [`author_matches`]).
    pub fn matches(&self, c: &CommitInfo) -> bool {
        let q = self.text.as_str();
        let text = q.is_empty()
            || contains_lower(&c.summary, q)
            || contains_lower(&c.body, q)
            || contains_lower(&c.author, q)
            || hex_starts_with(c.oid.as_bytes(), q);
        text && (self.authors.is_empty() || self.authors.iter().any(|a| author_matches(a, c)))
    }

    /// Indexes of the matching commits, ascending; empty for an empty query.
    pub fn matching_rows(&self, commits: &[CommitInfo]) -> Vec<usize> {
        if self.is_empty() {
            return Vec::new();
        }
        commits.iter().enumerate().filter(|(_, c)| self.matches(c)).map(|(i, _)| i).collect()
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
/// `author` is `None`. The other words keep their order and spelling.
pub fn with_author(query: &str, author: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = author.map(str::trim).filter(|a| !a.is_empty()) {
        let needs_quotes = a.contains(char::is_whitespace) || a.starts_with('"');
        parts.push(if needs_quotes { format!("{AUTHOR}\"{}\"", a.replace('"', "")) } else { format!("{AUTHOR}{a}") });
    }
    for t in tokens(query) {
        if let Token::Word(w) = t {
            parts.push(w.to_owned());
        }
    }
    parts.join(" ")
}

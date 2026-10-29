//! One commit as the graph holds it, kept small: a large history holds hundreds of thousands.
//!
//! The author is shared: a walk reads each commit's signature once and looks its (name, email)
//! up in an [`Authors`] table, so every commit by the same person points at one copy of the
//! two strings. Texts are shared on clone (a reload that takes commits from the one before
//! points at the same text instead of copying it), with no spare capacity, and the usual single
//! parent is kept in the commit itself.
//!
//! The body is held whole, once, flattened to one line: search matches all of it, and the graph
//! row and its hover show the start of it (`body_preview`).

use git2::Oid;
use smallvec::SmallVec;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

/// A commit's parents, first parent first; one is kept inline, merges spill to the heap.
pub type Parents = SmallVec<[Oid; 1]>;

#[derive(Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub summary: Text,
    /// The whole message body flattened to one line: what search matches. The graph shows its
    /// start (`body_preview`) dimmed after the summary.
    pub body: Text,
    /// Name and email from the author signature, shared with the other commits by them.
    pub author: Author,
    /// Committer time, in seconds.
    pub time: i64,
    pub parents: Parents,
}

/// How many bytes of the body the graph shows before it cuts the preview at the next word.
pub const BODY_PREVIEW: usize = 240;

impl CommitInfo {
    /// The start of the body the graph shows: the whole flattened body up to the end of the
    /// first word that reaches past `BODY_PREVIEW` bytes, and whether more follows (shown as
    /// "…"). Borrowed from the body, so the preview costs no memory of its own.
    pub fn body_preview(&self) -> (&str, bool) {
        body_preview(&self.body)
    }
}

/// `CommitInfo::body_preview` of the flattened body `body` (words joined by single spaces).
pub fn body_preview(body: &str) -> (&str, bool) {
    // The words are joined by ASCII spaces, so a space past the limit is a char boundary.
    match body.as_bytes().get(BODY_PREVIEW + 1..).and_then(|rest| memchr::memchr(b' ', rest)) {
        Some(i) => (&body[..BODY_PREVIEW + 1 + i], true),
        None => (body, false),
    }
}

/// A string that can't grow, shared on clone: two words like a `Box<str>`, no spare capacity,
/// and every empty text points at one shared allocation. Reads as a `str`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text(Arc<str>);

/// The one allocation every empty `Text` shares (most commits have no body).
static EMPTY: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(""));

impl Default for Text {
    fn default() -> Text {
        Text(Arc::clone(&EMPTY))
    }
}

impl Text {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `self` and `other` are the same shared copy (not just equal).
    pub fn ptr_eq(&self, other: &Text) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Text {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        if s.is_empty() { Text::default() } else { Text(s.into()) }
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::from(s.as_str())
    }
}

impl From<&Text> for String {
    fn from(t: &Text) -> String {
        t.0.as_ref().to_owned()
    }
}

impl From<Text> for String {
    fn from(t: Text) -> String {
        t.0.as_ref().to_owned()
    }
}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.0, f)
    }
}

/// A commit author's name and email, shared by reference counting. Two authors are equal when
/// both strings are.
#[derive(Clone)]
pub struct Author(Arc<Person>);

struct Person {
    /// The name, a NUL, then the email: git keeps NULs out of signatures, so this is the key
    /// [`Authors`] looks authors up by.
    key: Box<str>,
    name_len: usize,
}

impl Author {
    /// A new, unshared author. A walk interns them through [`Authors`] instead.
    pub fn new(name: &str, email: &str) -> Author {
        let mut key = String::with_capacity(name.len() + 1 + email.len());
        push_key(&mut key, name, email);
        Author(Arc::new(Person { key: key.into_boxed_str(), name_len: name.len() }))
    }

    pub fn name(&self) -> &str {
        &self.0.key[..self.0.name_len]
    }

    pub fn email(&self) -> &str {
        &self.0.key[self.0.name_len + 1..]
    }

    /// Whether `self` and `other` are the same shared copy (not just equal).
    pub fn ptr_eq(&self, other: &Author) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Puts the key of (`name`, `email`) in `out`: what a table of authors is searched by
    /// (`Author: Borrow<str>`).
    pub(crate) fn key_into(out: &mut String, name: &str, email: &str) {
        out.clear();
        push_key(out, name, email);
    }
}

fn push_key(out: &mut String, name: &str, email: &str) {
    out.push_str(name);
    out.push('\0');
    out.push_str(email);
}

impl PartialEq for Author {
    fn eq(&self, other: &Author) -> bool {
        self.ptr_eq(other) || self.0.key == other.0.key
    }
}

impl Eq for Author {}

/// Hashes as its key (`Borrow<str>`), so a table of authors can be searched by a key string.
impl Hash for Author {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.key.hash(h);
    }
}

impl Borrow<str> for Author {
    fn borrow(&self) -> &str {
        &self.0.key
    }
}

impl std::fmt::Debug for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <{}>", self.name(), self.email())
    }
}

/// The authors met so far, so each (name, email) is stored once.
#[derive(Default)]
pub struct Authors {
    known: HashSet<Author>,
    /// Scratch space for the lookup key.
    key: String,
}

impl Authors {
    /// The shared author with this name and email, added on first sight.
    pub fn get(&mut self, name: &str, email: &str) -> Author {
        Author::key_into(&mut self.key, name, email);
        if let Some(a) = self.known.get(self.key.as_str()) {
            return a.clone();
        }
        let a = Author::new(name, email);
        self.known.insert(a.clone());
        a
    }

    /// Adds `a` itself, so later lookups of that name and email share its copy; an equal author
    /// already held is kept.
    pub fn seed(&mut self, a: &Author) {
        if !self.known.contains(a) {
            self.known.insert(a.clone());
        }
    }

    /// How many distinct authors it holds.
    pub fn len(&self) -> usize {
        self.known.len()
    }

    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }
}

#[cfg(test)]
#[path = "commit_info_tests.rs"]
mod tests;

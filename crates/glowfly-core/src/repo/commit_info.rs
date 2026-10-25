//! One commit as the graph holds it, kept small: a large history holds hundreds of thousands.
//!
//! The author is shared: a walk reads each commit's signature once and looks its (name, email)
//! up in an [`Authors`] table, so every commit by the same person points at one copy of the
//! two strings. Texts are boxed (no spare capacity) and the usual single parent is kept in the
//! commit itself.

use git2::Oid;
use smallvec::SmallVec;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A commit's parents, first parent first; one is kept inline, merges spill to the heap.
pub type Parents = SmallVec<[Oid; 1]>;

#[derive(Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub summary: Text,
    /// Message body flattened to one line (shown dimmed after the summary).
    pub body: Text,
    /// Name and email from the author signature, shared with the other commits by them.
    pub author: Author,
    /// Committer time, in seconds.
    pub time: i64,
    pub parents: Parents,
}

/// A string that can't grow: two words instead of a `String`'s three, and no spare capacity.
/// Reads as a `str`.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text(Box<str>);

impl Text {
    pub fn as_str(&self) -> &str {
        &self.0
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
        Text(s.into())
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text(s.into_boxed_str())
    }
}

impl From<&Text> for String {
    fn from(t: &Text) -> String {
        t.0.as_ref().to_owned()
    }
}

impl From<Text> for String {
    fn from(t: Text) -> String {
        t.0.into_string()
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

//! The mailmap: which name and email the graph, the commit details and search show for a
//! commit's author and committer, as `log.mailmap` (on by default) has git show them.
//!
//! Glowfly reads the three sources itself, in git's order with later ones overriding: `.mailmap`
//! at the work tree root, `mailmap.blob` and `mailmap.file`. libgit2's own reader follows links
//! and reads without a limit or a look at where a path leads, which a load of an untrusted
//! repository mustn't do (docs/PLATFORMS.md, rule 1), so libgit2's reader is not called; and its
//! matcher compares letter case exactly where git does not, so the text read here is parsed and
//! matched by Glowfly's own table ([`Mailmap`]). A `.mailmap` that is not a plain file is left out, as git leaves out
//! a linked one; `mailmap.file` is read only where `net_path::may_follow` allows from the work
//! tree; a source over [`MAX_BYTES`] or holding a NUL is left out too.
//!
//! [`MailmapText`] is the text one load read: plain data, cheap to clone and to send; each reader
//! opens its own [`Mailmap`] from it. Matching is git's: an email, and a commit name where an
//! entry gives one, are compared without regard to ASCII letter case (other characters as
//! written); a later line overrides an earlier one, also when they differ only in case; what is
//! shown is spelled as the entry writes it, and a field an entry doesn't give stays as the commit
//! writes it.

use crate::commit_text::Person;
use crate::config_includes::read_file_bounded;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

/// The most one mailmap source may hold, in bytes.
const MAX_BYTES: u64 = 1 << 20;

/// The mailmap text one load read, the sources joined; empty when `log.mailmap` is off or there
/// is none.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MailmapText {
    text: Arc<str>,
    stamp: u64,
}

/// Prints the length only: effects holding it are logged, and names and emails stay out of logs.
impl std::fmt::Debug for MailmapText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MailmapText({} bytes)", self.text.len())
    }
}

impl MailmapText {
    /// The mailmap of `repo`'s work tree, read from its sources (a few bounded file reads).
    pub fn read(repo: &git2::Repository) -> MailmapText {
        let sources = Sources::of(repo);
        let mut text = String::new();
        let mut add = |bytes: Option<Vec<u8>>| {
            if let Some(b) = bytes.filter(|b| !b.contains(&0)) {
                text.push_str(&String::from_utf8_lossy(&b));
                text.push('\n');
            }
        };
        add(sources.tree_file.and_then(|p| read_file_bounded(&p, MAX_BYTES)));
        add(sources.blob.and_then(|rev| blob_bytes(repo, &rev)));
        add(sources.file.and_then(|p| read_file_bounded(&p, MAX_BYTES)));
        MailmapText::from_text(&text)
    }

    /// The mailmap that `text` holds (no file is read).
    pub fn from_text(text: &str) -> MailmapText {
        if text.trim().is_empty() {
            return MailmapText::default();
        }
        let mut h = DefaultHasher::new();
        text.hash(&mut h);
        MailmapText { text: Arc::from(text), stamp: h.finish().max(1) }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// A number for this text: 0 when empty, and a different one for a different text. Only
    /// meaningful compared with another taken in the same run of the app.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    /// Whether `self` and `other` hold the same shared copy of the text.
    pub fn ptr_eq(&self, other: &MailmapText) -> bool {
        Arc::ptr_eq(&self.text, &other.text)
    }

    /// The text parsed into a table to resolve people with.
    pub fn open(&self) -> Mailmap {
        Mailmap::parse(&self.text)
    }
}

/// What an entry shows in place of what a commit wrote; None keeps the written one.
#[derive(Default)]
struct Shown {
    name: Option<Box<str>>,
    email: Option<Box<str>>,
}

/// The entries for one commit email.
#[derive(Default)]
struct Entry {
    /// For any name under the email; a later line sets only the fields it gives.
    any: Shown,
    /// For one commit name under the email (key: the name, folded); a later line replaces it whole.
    named: HashMap<Box<str>, Shown>,
}

/// Glowfly's own table of mailmap entries (no `Debug`: names and emails stay out of logs).
#[derive(Default)]
pub struct Mailmap {
    /// Key: the commit email, folded.
    by_email: HashMap<Box<str>, Entry>,
}

/// `s` with ASCII letters in lower case, as git compares a name or an email (other characters
/// as written); borrowed when nothing changes.
fn folded(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) { Cow::Owned(s.to_ascii_lowercase()) } else { Cow::Borrowed(s) }
}

/// git's own white space: space, TAB, CR and LF only.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// One `Name <email>` of a line: the name trimmed (None when empty), the email as written, and
/// the text after `>`. None without a `<...>`, or with an empty one unless `empty_ok`.
fn name_and_email(s: &str, empty_ok: bool) -> Option<(Option<&str>, &str, &str)> {
    let left = s.find('<')?;
    let right = left + 1 + s[left + 1..].find('>')?;
    if !empty_ok && right == left + 1 {
        return None;
    }
    let name = s[..left].trim_matches(is_space);
    Some(((!name.is_empty()).then_some(name), &s[left + 1..right], &s[right + 1..]))
}

impl Mailmap {
    /// A mailmap that leaves everyone as written.
    pub fn none() -> Mailmap {
        Mailmap::default()
    }

    pub fn is_none(&self) -> bool {
        self.by_email.is_empty()
    }

    fn parse(text: &str) -> Mailmap {
        let mut map = Mailmap::default();
        for line in text.split('\n') {
            map.add_line(line);
        }
        map
    }

    /// One line, read as git reads it: only a first-byte `#` makes a comment.
    fn add_line(&mut self, line: &str) {
        if line.starts_with('#') {
            return;
        }
        let Some((name, email, rest)) = name_and_email(line, false) else { return };
        match name_and_email(rest, true) {
            Some((Some(old_name), old_email, _)) => {
                let shown = Shown { name: name.map(Into::into), email: Some(email.into()) };
                self.entry(old_email).named.insert(folded(old_name).into_owned().into_boxed_str(), shown);
            }
            Some((None, old_email, _)) => {
                let any = &mut self.entry(old_email).any;
                if let Some(name) = name {
                    any.name = Some(name.into());
                }
                any.email = Some(email.into());
            }
            None => {
                if let Some(name) = name {
                    self.entry(email).any.name = Some(name.into());
                }
            }
        }
    }

    fn entry(&mut self, email: &str) -> &mut Entry {
        self.by_email.entry(folded(email).into_owned().into_boxed_str()).or_default()
    }

    /// The name and email the mailmap shows for `name` and `email`: the entry for the name under
    /// the email first, then the one for the email alone, as git does. None when no entry applies
    /// or the entry says what the commit already does.
    pub fn resolve(&self, name: &str, email: &str) -> Option<(String, String)> {
        let entry = self.by_email.get(folded(email).as_ref())?;
        let named = if entry.named.is_empty() { None } else { entry.named.get(folded(name).as_ref()) };
        let shown = named.unwrap_or(&entry.any);
        let (n, e) = (shown.name.as_deref().unwrap_or(name), shown.email.as_deref().unwrap_or(email));
        (n != name || e != email).then(|| (n.to_owned(), e.to_owned()))
    }

    /// `p` as the mailmap shows it.
    pub fn person(&self, p: Person) -> Person {
        match self.resolve(&p.name, &p.email) {
            Some((name, email)) => Person { name, email },
            None => p,
        }
    }
}

/// Where a repository's mailmap comes from, as far as Glowfly reads it.
pub(crate) struct Sources {
    /// `.mailmap` at the work tree root, when it is a plain file.
    pub tree_file: Option<PathBuf>,
    /// `mailmap.blob`: a revision naming a blob.
    pub blob: Option<String>,
    /// `mailmap.file`, absolute, when it may be followed from the work tree and is a plain file.
    pub file: Option<PathBuf>,
}

impl Sources {
    /// The sources of `repo`: none with `log.mailmap` off (any config level) or without a work
    /// tree.
    pub(crate) fn of(repo: &git2::Repository) -> Sources {
        let none = Sources { tree_file: None, blob: None, file: None };
        let (Some(workdir), Ok(config)) = (repo.workdir(), repo.config()) else { return none };
        if !config.get_bool("log.mailmap").unwrap_or(true) {
            return none;
        }
        // git doesn't follow a linked `.mailmap` in the work tree.
        let tree_file = Some(workdir.join(".mailmap")).filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file()));
        let blob = config.get_string("mailmap.blob").ok().filter(|b| !b.is_empty());
        let file = config
            .get_path("mailmap.file")
            .ok()
            .map(|p| if p.is_absolute() { p } else { workdir.join(p) })
            .filter(|p| p.is_absolute() && crate::net_path::may_follow(p, workdir) && std::fs::metadata(p).is_ok_and(|m| m.is_file()));
        Sources { tree_file, blob, file }
    }
}

/// The bytes of the blob `rev` names, when it is at most [`MAX_BYTES`].
fn blob_bytes(repo: &git2::Repository, rev: &str) -> Option<Vec<u8>> {
    let blob = repo.revparse_single(rev).ok()?.peel_to_blob().ok()?;
    (blob.size() as u64 <= MAX_BYTES).then(|| blob.content().to_vec())
}

#[cfg(test)]
#[path = "mailmap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "mailmap_case_tests.rs"]
mod case_tests;

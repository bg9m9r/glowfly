//! The mailmap: which name and email the graph, the commit details and search show for a
//! commit's author and committer, as `log.mailmap` (on by default) has git show them.
//!
//! Glowfly reads the three sources itself, in git's order with later ones overriding: `.mailmap`
//! at the work tree root, `mailmap.blob` and `mailmap.file`. libgit2's own reader follows links
//! and reads without a limit or a look at where a path leads, which a load of an untrusted
//! repository mustn't do (docs/PLATFORMS.md, rule 1), so the text read here goes to libgit2's
//! parser and resolver only. A `.mailmap` that is not a plain file is left out, as git leaves out
//! a linked one; `mailmap.file` is read only where `net_path::may_follow` allows from the work
//! tree; a source over [`MAX_BYTES`] or holding a NUL is left out too.
//!
//! [`MailmapText`] is the text one load read: plain data, cheap to clone and to send to another
//! thread. libgit2's parsed object isn't `Send`, so each thread opens its own ([`Mailmap`]).
//! Matching is libgit2's: exact, where git ignores the case of a name or an email.

use crate::commit_text::Person;
use crate::config_includes::read_file_bounded;
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

    /// The text parsed, for the calling thread to resolve people with.
    pub fn open(&self) -> Mailmap {
        if self.is_empty() {
            return Mailmap::none();
        }
        Mailmap(git2::Mailmap::from_buffer(&self.text).ok())
    }
}

/// A parsed mailmap, for one thread (libgit2's object isn't `Send`).
pub struct Mailmap(Option<git2::Mailmap>);

impl Mailmap {
    /// A mailmap that leaves everyone as written.
    pub fn none() -> Mailmap {
        Mailmap(None)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    /// The name and email the mailmap shows for `name` and `email`; None when it leaves them as
    /// written. That includes a signature libgit2 can't build (an empty name or email, or
    /// `<` or `>` inside) and one it would trim without any entry matching.
    pub fn resolve(&self, name: &str, email: &str) -> Option<(String, String)> {
        let map = self.0.as_ref()?;
        // Built the way libgit2 builds the answer, so the trimming it does shows in both.
        let sig = git2::Signature::new(name, email, &git2::Time::new(0, 0)).ok()?;
        let real = map.resolve_signature(&sig).ok()?;
        let (real_name, real_email) = (real.name()?, real.email()?);
        (Some(real_name) != sig.name() || Some(real_email) != sig.email()).then(|| (real_name.to_owned(), real_email.to_owned()))
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

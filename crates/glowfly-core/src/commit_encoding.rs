//! Commit messages written in the encoding the repository asks for (`i18n.commitEncoding`).
//! git writes a message file's bytes as they are and labels the commit with that encoding, so
//! every message Glowfly hands git in a file (`commit -F`, a rebase's reword and squash files)
//! is encoded into it first (`MessageEncoding::encode`); a character the encoding can't hold
//! refuses the write instead of becoming `?` or a character reference. Read with libgit2 from
//! the repository's whole config (system, global, local and the files they include): a value
//! git itself uses for every commit, never a command, so no trust check applies to it. A label
//! Glowfly doesn't decode (unknown ones, UTF-8, UTF-16) writes UTF-8, as before.
//!
//! Not covered, because git takes them on its command line, which Git for Windows always reads
//! as UTF-8: `stash push -m`, `tag -a -m` (tags have no encoding header, so UTF-8 is right) and
//! the init dialog's fixed ASCII first commit. `commit_text` reads valid UTF-8 under a single-byte
//! label as UTF-8, so those, and messages written before #242, still show as typed.

use crate::commit_text::{Decoder, known};
use crate::error::Error;
use encoding_rs::Encoding;
use git2::Repository;
use std::borrow::Cow;
use std::path::PathBuf;

/// How many distinct characters an [`Unencodable`] names at most.
const MAX_NAMED: usize = 8;

/// The encoding commit messages are written in. Copy; the default is UTF-8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageEncoding(Option<&'static Encoding>);

impl MessageEncoding {
    /// UTF-8: no `i18n.commitEncoding`, or one Glowfly writes as UTF-8.
    pub const UTF8: MessageEncoding = MessageEncoding(None);

    /// The repository's `i18n.commitEncoding`, through its effective config with includes.
    pub fn of(repo: &Repository) -> MessageEncoding {
        let label = repo.config().and_then(|mut c| c.snapshot()).and_then(|c| c.get_string("i18n.commitEncoding")).ok();
        MessageEncoding::named(label.as_deref())
    }

    /// For an `i18n.commitEncoding` value (None: unset).
    pub fn named(label: Option<&str>) -> MessageEncoding {
        MessageEncoding(label.and_then(known))
    }

    /// The encoding's WHATWG name ("windows-1252", "Shift_JIS"), "UTF-8" for UTF-8.
    pub fn name(self) -> &'static str {
        self.0.map_or("UTF-8", Encoding::name)
    }

    /// Whether messages are written as UTF-8.
    pub fn is_utf8(self) -> bool {
        self.0.is_none()
    }

    /// `text` in this encoding, or which characters it can't hold. Borrows for UTF-8 and ASCII.
    pub fn encode(self, text: &str) -> Result<Cow<'_, [u8]>, Unencodable> {
        let Some(enc) = self.0 else { return Ok(Cow::Borrowed(text.as_bytes())) };
        if text.is_ascii() && enc.is_ascii_compatible() {
            return Ok(Cow::Borrowed(text.as_bytes()));
        }
        let (bytes, _, had_errors) = enc.encode(text);
        if !had_errors {
            return Ok(bytes);
        }
        let mut chars: Vec<char> = Vec::new();
        for c in text.chars() {
            if chars.len() < MAX_NAMED && !chars.contains(&c) && enc.encode(c.encode_utf8(&mut [0; 4])).2 {
                chars.push(c);
            }
        }
        Err(Unencodable { encoding: enc.name(), chars })
    }

    /// Text Glowfly wrote with [`encode`](Self::encode), read back (a rebase's message files).
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        Decoder::with(self.0).decode(bytes)
    }

    /// Checks every message in `texts`, failing on the first that doesn't fit.
    pub fn check<'a>(self, texts: impl IntoIterator<Item = &'a str>) -> Result<(), Unencodable> {
        texts.into_iter().try_for_each(|t| self.encode(t).map(drop))
    }

    /// Writes each message file `(path, text)` encoded, after encoding them all, so a message
    /// that doesn't fit (`Error::Unsupported`, naming its characters) writes none of them.
    pub fn write_files(self, files: &[(PathBuf, String)]) -> crate::error::Result<()> {
        let encoded: Vec<(&PathBuf, Cow<'_, [u8]>)> = files
            .iter()
            .map(|(path, text)| self.encode(text).map(|bytes| (path, bytes)))
            .collect::<Result<_, _>>()
            .map_err(|e| Error::Unsupported(e.to_string()))?;
        for (path, bytes) in encoded {
            crate::persist::atomic_write(path, &bytes)?;
        }
        Ok(())
    }
}

/// A message holds characters the repository's commit encoding can't represent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unencodable {
    /// The encoding's name.
    pub encoding: &'static str,
    /// The first few distinct characters it can't hold, in message order.
    pub chars: Vec<char>,
}

impl Unencodable {
    /// The characters, quoted and separated by spaces: `"✓" "日"`.
    pub fn listed(&self) -> String {
        self.chars.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(" ")
    }
}

impl std::fmt::Display for Unencodable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "This repository writes commit messages in {} (i18n.commitEncoding), which can't hold {}. Remove them from the message.",
            self.encoding,
            self.listed()
        )
    }
}

#[cfg(test)]
#[path = "commit_encoding_tests.rs"]
mod tests;

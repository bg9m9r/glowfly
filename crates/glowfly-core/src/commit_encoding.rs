//! Commit messages written in the encoding the repository asks for (`i18n.commitEncoding`).
//! git writes a message file's bytes as they are and labels the commit with that encoding, so
//! every message Glowfly hands git in a file (`commit -F`, a rebase's reword and squash files)
//! is encoded into it first (`MessageEncoding::encode`); a character the encoding can't hold
//! refuses the write instead of becoming `?` or a character reference. git records the label as
//! configured and readers take it strictly (iconv), so a label WHATWG maps to a wider code page
//! (`ISO-8859-1` and `US-ASCII` to windows-1252, and a few more: `Strict`) also refuses the
//! characters only the wider page has, such as `€` and `’` under ISO-8859-1. Read with libgit2 from
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
use encoding_rs::{Encoding, WINDOWS_874, WINDOWS_1252, WINDOWS_1254};
use git2::Repository;
use std::borrow::Cow;
use std::path::PathBuf;

/// How many distinct characters an [`Unencodable`] names at most.
const MAX_NAMED: usize = 8;

/// The encoding commit messages are written in, with the label as configured. The default is
/// UTF-8.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageEncoding {
    enc: Option<&'static Encoding>,
    /// `i18n.commitEncoding` as written (trimmed): what git records in the header.
    label: String,
    strict: Strict,
}

/// What a label git records allows beyond its WHATWG encoding's encoder. WHATWG maps several
/// labels to a wider Windows code page (`ISO-8859-1` and `US-ASCII` to windows-1252,
/// `ISO-8859-9` to windows-1254, `ISO-8859-11` and `TIS-620` to windows-874), while git and
/// every reader using iconv take the label strictly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Strict {
    /// The encoder is the label's own.
    #[default]
    No,
    /// ISO-8859-1: U+0000 to U+00FF only (windows-1252's 0x80 to 0x9F letters are C1 controls
    /// there).
    Latin1,
    /// US-ASCII: ASCII only.
    Ascii,
    /// The code page's bytes in this range mean something else (or nothing) under the label.
    NoBytes(u8, u8),
}

impl Strict {
    fn of(label: &str, enc: &'static Encoding) -> Strict {
        let l = label.to_ascii_lowercase();
        let l = l.as_str();
        if enc == WINDOWS_1252 {
            match l {
                "windows-1252" | "cp1252" | "x-cp1252" => Strict::No,
                "ascii" | "us-ascii" | "ansi_x3.4-1968" => Strict::Ascii,
                _ => Strict::Latin1,
            }
        } else if enc == WINDOWS_1254 && !matches!(l, "windows-1254" | "cp1254" | "x-cp1254") {
            Strict::NoBytes(0x80, 0x9f)
        } else if enc == WINDOWS_874 {
            match l {
                "windows-874" | "dos-874" => Strict::No,
                // TIS-620 has no 0xA0 either.
                "tis-620" => Strict::NoBytes(0x80, 0xa0),
                _ => Strict::NoBytes(0x80, 0x9f),
            }
        } else {
            Strict::No
        }
    }
}

impl MessageEncoding {
    /// UTF-8: no `i18n.commitEncoding`, or one Glowfly writes as UTF-8.
    pub const UTF8: MessageEncoding = MessageEncoding { enc: None, label: String::new(), strict: Strict::No };

    /// The repository's `i18n.commitEncoding`, through its effective config with includes.
    pub fn of(repo: &Repository) -> MessageEncoding {
        let label = repo.config().and_then(|mut c| c.snapshot()).and_then(|c| c.get_string("i18n.commitEncoding")).ok();
        MessageEncoding::named(label.as_deref())
    }

    /// For an `i18n.commitEncoding` value (None: unset).
    pub fn named(label: Option<&str>) -> MessageEncoding {
        let Some((label, enc)) = label.map(str::trim).and_then(|l| known(l).map(|e| (l, e))) else { return MessageEncoding::UTF8 };
        MessageEncoding { enc: Some(enc), label: label.to_owned(), strict: Strict::of(label, enc) }
    }

    /// The label as configured ("ISO-8859-1", "cp932"), "UTF-8" for UTF-8.
    pub fn name(&self) -> &str {
        if self.enc.is_some() { &self.label } else { "UTF-8" }
    }

    /// Whether messages are written as UTF-8.
    pub fn is_utf8(&self) -> bool {
        self.enc.is_none()
    }

    /// Whether `c` has bytes under the configured label.
    fn fits(&self, enc: &'static Encoding, c: char) -> bool {
        match self.strict {
            Strict::Latin1 => (c as u32) < 0x80 || ((c as u32) >= 0xa0 && (c as u32) <= 0xff),
            Strict::Ascii => c.is_ascii(),
            Strict::No | Strict::NoBytes(..) => {
                let mut buf = [0; 4];
                let (bytes, _, had_errors) = enc.encode(c.encode_utf8(&mut buf));
                let banned = |b: &u8| matches!(self.strict, Strict::NoBytes(lo, hi) if (lo..=hi).contains(b));
                !had_errors && !(bytes.len() == 1 && banned(&bytes[0]))
            }
        }
    }

    /// `text` in this encoding, or which characters it can't hold under the configured label.
    /// Borrows for UTF-8 and ASCII.
    pub fn encode<'t>(&self, text: &'t str) -> Result<Cow<'t, [u8]>, Unencodable> {
        let Some(enc) = self.enc else { return Ok(Cow::Borrowed(text.as_bytes())) };
        if text.is_ascii() && enc.is_ascii_compatible() {
            return Ok(Cow::Borrowed(text.as_bytes()));
        }
        if self.strict == Strict::No || text.chars().all(|c| self.fits(enc, c)) {
            let (bytes, _, had_errors) = enc.encode(text);
            if !had_errors {
                return Ok(bytes);
            }
        }
        let mut chars: Vec<char> = Vec::new();
        for c in text.chars() {
            if chars.len() < MAX_NAMED && !chars.contains(&c) && !self.fits(enc, c) {
                chars.push(c);
            }
        }
        Err(Unencodable { encoding: self.label.clone(), chars })
    }

    /// Text Glowfly wrote with [`encode`](Self::encode), read back (a rebase's message files).
    pub fn decode<'b>(&self, bytes: &'b [u8]) -> Cow<'b, str> {
        Decoder::with(self.enc).decode(bytes)
    }

    /// Checks every message in `texts`, failing on the first that doesn't fit.
    pub fn check<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> Result<(), Unencodable> {
        texts.into_iter().try_for_each(|t| self.encode(t).map(drop))
    }

    /// Writes each message file `(path, text)` encoded, after encoding them all, so a message
    /// that doesn't fit (`Error::Unsupported`, naming its characters) writes none of them.
    pub fn write_files(&self, files: &[(PathBuf, String)]) -> crate::error::Result<()> {
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
    /// The encoding's label, as configured.
    pub encoding: String,
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

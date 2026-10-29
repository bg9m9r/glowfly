//! Commit text read from the commit's bytes: decoded with the encoding its `encoding` header
//! names (git writes one when `i18n.commitEncoding` is set) and as UTF-8 otherwise. Text that
//! doesn't decode gets U+FFFD in place of the bad bytes instead of coming out empty. Every reader
//! of a commit's message, summary, body or people goes through here; writing goes through
//! `commit_encoding`.

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use git2::{Commit, Signature};
use std::borrow::Cow;

/// iconv names git users write in i18n.commitEncoding that the WHATWG label list lacks.
const ALIASES: &[(&str, &str)] = &[
    ("latin-1", "latin1"),
    ("cp932", "shift_jis"),
    ("eucjp", "euc-jp"),
    ("euc_jp", "euc-jp"),
    ("euckr", "euc-kr"),
    ("cp949", "euc-kr"),
    ("cp936", "gbk"),
    ("cp950", "big5"),
];

/// How one commit's text decodes (None: UTF-8). Copy, so a hot loop reads the header once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoder(Option<&'static Encoding>);

impl Decoder {
    /// The decoder for the commit's own `encoding` header.
    pub fn of(c: &Commit<'_>) -> Decoder {
        Decoder::named(c.message_encoding())
    }

    /// For an `encoding` header's value; unknown, UTF-8 and UTF-16 labels decode as UTF-8.
    pub fn named(label: Option<&str>) -> Decoder {
        Decoder(label.and_then(known))
    }

    /// For an encoding already looked up (None: UTF-8).
    pub(crate) fn with(enc: Option<&'static Encoding>) -> Decoder {
        Decoder(enc)
    }

    /// `bytes` as text: in the named encoding, or as UTF-8 when there is none or when the bytes
    /// are malformed in it but valid UTF-8 (a wrong header). Valid UTF-8 with non-ASCII under a
    /// single-byte label reads as UTF-8 too: that is what git writes for `commit -m` and
    /// `stash -m` there (it doesn't re-encode them) and what Glowfly wrote before #242, while real
    /// single-byte text is almost never valid UTF-8. So an amend of such a commit doesn't stack
    /// another layer of mojibake. Multi-byte labels keep their decoding: real text in them can
    /// be valid UTF-8 as well. Borrows when nothing changes.
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        let Some(enc) = self.0 else { return String::from_utf8_lossy(bytes) };
        if enc.is_single_byte()
            && !bytes.is_ascii()
            && let Ok(s) = std::str::from_utf8(bytes)
        {
            return Cow::Borrowed(s);
        }
        let (text, had_errors) = enc.decode_without_bom_handling(bytes);
        if had_errors && let Ok(s) = std::str::from_utf8(bytes) {
            return Cow::Borrowed(s);
        }
        text
    }

    /// The commit's first paragraph, whitespace squashed (empty when it has none).
    pub fn summary<'a>(self, c: &'a Commit<'_>) -> Cow<'a, str> {
        self.decode(c.summary_bytes().unwrap_or_default())
    }

    /// The commit's whole message.
    pub fn message<'a>(self, c: &'a Commit<'_>) -> Cow<'a, str> {
        self.decode(c.message_bytes())
    }

    /// The commit's message after the summary paragraph (empty when it has none).
    pub fn body<'a>(self, c: &'a Commit<'_>) -> Cow<'a, str> {
        self.decode(c.body_bytes().unwrap_or_default())
    }

    /// A signature's name, from the commit this decoder was made for.
    pub fn name<'s>(self, sig: &'s Signature<'_>) -> Cow<'s, str> {
        self.decode(sig.name_bytes())
    }

    /// A signature's email, from the commit this decoder was made for.
    pub fn email<'s>(self, sig: &'s Signature<'_>) -> Cow<'s, str> {
        self.decode(sig.email_bytes())
    }
}

/// The encoding a label names, if Glowfly decodes it as something other than UTF-8.
pub(crate) fn known(label: &str) -> Option<&'static Encoding> {
    let label = label.trim();
    let label = ALIASES.iter().find(|(a, _)| a.eq_ignore_ascii_case(label)).map_or(label, |&(_, w)| w);
    Encoding::for_label_no_replacement(label.as_bytes()).filter(|&e| e != UTF_8 && e != UTF_16LE && e != UTF_16BE)
}

/// A signature's name and email, decoded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub email: String,
}

impl Person {
    /// `sig`'s name and email, decoded with `d`.
    fn of(d: Decoder, sig: &Signature<'_>) -> Person {
        Person { name: d.name(sig).into_owned(), email: d.email(sig).into_owned() }
    }
}

/// The commit's summary, decoded.
pub fn summary<'a>(c: &'a Commit<'_>) -> Cow<'a, str> {
    Decoder::of(c).summary(c)
}

/// The commit's whole message, decoded.
pub fn message<'a>(c: &'a Commit<'_>) -> Cow<'a, str> {
    Decoder::of(c).message(c)
}

/// The commit's body (the message after the summary), decoded.
pub fn body<'a>(c: &'a Commit<'_>) -> Cow<'a, str> {
    Decoder::of(c).body(c)
}

/// The commit's author, decoded.
pub fn author(c: &Commit<'_>) -> Person {
    Person::of(Decoder::of(c), &c.author())
}

/// The commit's committer, decoded.
pub fn committer(c: &Commit<'_>) -> Person {
    Person::of(Decoder::of(c), &c.committer())
}

#[cfg(test)]
#[path = "commit_text_tests.rs"]
mod tests;

//! The UI's translations: the languages Trellis ships, their bundled catalogs (English is the
//! base; each other language is a JSON file of the same keys, `locales/`), lookups that fall
//! back to English and then to the key itself, `{name}` placeholders filled in one pass, and the
//! OS language, read once at startup (`system.rs`).
//!
//! Views and update build text with `settings.lang()` and `tr` / `trf` / `trn`. Translated text
//! is shown, never matched against git output or used as an id.

mod catalog;
mod system;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::fmt::Display;

pub use system::{detect_system_language, pick, system};

/// The Language setting's value for "follow the OS".
pub const SYSTEM: &str = "system";

/// A language with a bundled catalog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    #[default]
    En,
    De,
    Es,
}

impl Lang {
    /// Every language, in menu order: sorted by its own name.
    pub const ALL: [Lang; 3] = [Lang::De, Lang::En, Lang::Es];

    /// The code saved in the settings.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Es => "es",
        }
    }

    /// The language's name in that language (never translated).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
        }
    }

    /// The language saved as `code` (exact, lowercase), if Trellis has it.
    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }

    /// The language a setting asks for: its code, or the OS language for `SYSTEM`, "" and codes
    /// this version doesn't know.
    pub fn from_setting(setting: &str) -> Lang {
        Lang::from_code(setting).unwrap_or_else(system)
    }

    /// The text of `key`, in English when this language lacks it, or the key itself when English
    /// lacks it too.
    pub fn tr(self, key: &'static str) -> &'static str {
        lookup(catalog::table(self), catalog::table(Lang::En), key)
    }

    /// `tr`, with its `{name}` placeholders filled from `args`.
    pub fn trf(self, key: &'static str, args: &[(&str, &dyn Display)]) -> String {
        fill(self.tr(key), args)
    }

    /// The plural form of `key` for `n` (`key.one` or `key.other`), filled with `count` = `n`
    /// and `args`.
    pub fn trn(self, key: &'static str, n: usize, args: &[(&str, &dyn Display)]) -> String {
        let full = format!("{key}.{}", plural_suffix(n));
        let own = catalog::table(self);
        let base = catalog::table(Lang::En);
        let text = own.get(&full).or_else(|| base.get(&full)).map(String::as_str).unwrap_or(&full);
        let mut all: Vec<(&str, &dyn Display)> = vec![("count", &n)];
        all.extend_from_slice(args);
        fill(text, &all)
    }

    /// Whether this language's own file has `key` (no fallback).
    pub fn has(self, key: &str) -> bool {
        catalog::table(self).contains_key(key)
    }

    /// Every key and text of this language's own file.
    pub fn keys(self) -> impl Iterator<Item = (&'static str, &'static str)> {
        catalog::table(self).iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// `key`'s text in `own`, else in `base`, else `key` itself.
pub(crate) fn lookup<'a>(own: &'a HashMap<String, String>, base: &'a HashMap<String, String>, key: &'a str) -> &'a str {
    own.get(key).or_else(|| base.get(key)).map(String::as_str).unwrap_or(key)
}

/// The plural form for `n`: "one" for exactly one, else "other". Right for English, German and
/// Spanish; a language with more forms needs its own rule here.
pub fn plural_suffix(n: usize) -> &'static str {
    if n == 1 { "one" } else { "other" }
}

/// The `{name}` placeholders of `text` that `scan` finds, with their byte ranges.
fn scan(text: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            let len = bytes[start..].iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'_').count();
            let end = start + len;
            if len > 0 && bytes.get(end) == Some(&b'}') {
                out.push((i..end + 1, &text[start..end]));
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `text` with each `{name}` that `args` names replaced by its value, in one left-to-right pass:
/// inserted values are never scanned again, and unknown names, a lone `{` or `{}` stay as typed.
pub fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (range, name) in scan(text) {
        if let Some((_, value)) = args.iter().find(|(n, _)| *n == name) {
            out.push_str(&text[last..range.start]);
            out.push_str(&value.to_string());
            last = range.end;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// The names of `text`'s `{name}` placeholders.
pub fn placeholders(text: &str) -> BTreeSet<&str> {
    scan(text).into_iter().map(|(_, name)| name).collect()
}

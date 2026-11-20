//! What `git config --list -z` and `git config -z --get-regexp` print, and lookups in it as
//! git itself reads a key: the section and variable in any case, the subsection exactly, the
//! last value wins, and booleans by git's own rules. One list read replaces a `--get` per key.
//!
//! Each entry is the key, an LF and the value, ended by a NUL; a key written without `=` is the
//! bare key. The section and variable are printed lowercased, the subsection as written.

/// A config entry: its key and value; None for a key written without `=`.
pub type Entry = (String, Option<String>);

/// Reads `git config -z` output: entries end in NUL, each `key\nvalue` (the value may hold
/// newlines) or a bare `key` for a valueless entry.
pub fn parse_z(out: &str) -> Vec<Entry> {
    out.split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('\n') {
            Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
            None => (p.to_owned(), None),
        })
        .collect()
}

/// A key's section (up to the first dot), subsection (between the first and the last dot) and
/// variable (after the last dot). A key with no dot is all section.
fn split(key: &str) -> (&str, Option<&str>, &str) {
    match (key.find('.'), key.rfind('.')) {
        (Some(first), Some(last)) if first == last => (&key[..first], None, &key[last + 1..]),
        (Some(first), Some(last)) => (&key[..first], Some(&key[first + 1..last]), &key[last + 1..]),
        _ => (key, None, ""),
    }
}

/// Whether two keys name the same variable as git matches them: section and variable without
/// regard to ASCII case, the subsection exactly.
pub fn same_key(a: &str, b: &str) -> bool {
    let (sa, ua, va) = split(a);
    let (sb, ub, vb) = split(b);
    sa.eq_ignore_ascii_case(sb) && ua == ub && va.eq_ignore_ascii_case(vb)
}

/// A value as git reads a boolean (`git config --type=bool`): a valueless key is true, an empty
/// value false, then true/yes/on and false/no/off in any ASCII case (spaces are not trimmed),
/// else an integer (see [`parse_int`]) that is true unless zero. None when git refuses it.
pub fn parse_bool(value: Option<&str>) -> Option<bool> {
    let Some(v) = value else { return Some(true) };
    if v.is_empty() {
        return Some(false);
    }
    for word in ["true", "yes", "on"] {
        if v.eq_ignore_ascii_case(word) {
            return Some(true);
        }
    }
    for word in ["false", "no", "off"] {
        if v.eq_ignore_ascii_case(word) {
            return Some(false);
        }
    }
    parse_int(v).map(|n| n != 0)
}

/// An integer as git reads one (`strtoimax` with base 0, then at most one k/m/g suffix and
/// nothing after it) that fits in 32 bits once scaled.
fn parse_int(s: &str) -> Option<i32> {
    let s = s.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b');
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).filter(|r| r.starts_with(|c: char| c.is_ascii_hexdigit()));
    let (radix, digits) = match hex {
        Some(rest) => (16, rest),
        None if s.starts_with('0') => (8, s),
        None => (10, s),
    };
    let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let magnitude = i64::from_str_radix(&digits[..end], radix).ok()?;
    let factor: i64 = match &digits[end..] {
        "" => 1,
        "k" | "K" => 1024,
        "m" | "M" => 1024 * 1024,
        "g" | "G" => 1024 * 1024 * 1024,
        _ => return None,
    };
    let scaled = magnitude.checked_mul(factor)?;
    i32::try_from(if negative { -scaled } else { scaled }).ok()
}

/// The entries of one `git config --list -z` read, in git's reading order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigList {
    entries: Vec<Entry>,
}

impl ConfigList {
    /// The entries `git config ... -z` printed.
    pub fn parse(z: &str) -> Self {
        Self { entries: parse_z(z) }
    }

    /// Every entry, in order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The last value of `key`: None when it is unset, Some(None) when it is valueless.
    pub fn last(&self, key: &str) -> Option<Option<&str>> {
        self.entries.iter().rev().find(|(k, _)| same_key(k, key)).map(|(_, v)| v.as_deref())
    }

    /// The last value of `key` as `git config --get` prints it: a valueless key is "".
    pub fn text(&self, key: &str) -> Option<&str> {
        self.last(key).map(|v| v.unwrap_or(""))
    }

    /// Every value of `key`, in order (None for a valueless one).
    pub fn all(&self, key: &str) -> Vec<Option<&str>> {
        self.entries.iter().filter(|(k, _)| same_key(k, key)).map(|(_, v)| v.as_deref()).collect()
    }

    /// The last value of `key` read as a boolean; None when it is unset or git refuses it.
    pub fn bool(&self, key: &str) -> Option<bool> {
        self.last(key).and_then(parse_bool)
    }
}

#[cfg(test)]
#[path = "config_list_tests.rs"]
mod tests;

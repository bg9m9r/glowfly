//! The one place a git config entry's value is read. A key written without `=` (a bare key,
//! which git reads as true) has no value, and git2's `ConfigEntry::value`/`value_bytes` panic
//! on one. `crates/glowfly-app/tests/source_checks/config_values.rs` keeps every read here.

use std::borrow::Cow;

/// The entry's value as bytes; None for a bare key.
pub fn raw<'e>(e: &'e git2::ConfigEntry<'_>) -> Option<&'e [u8]> {
    // The only `value_bytes` call in the workspace: it panics when there is no value.
    e.has_value().then(|| e.value_bytes())
}

/// The entry's value as text; None for a bare key or a value that isn't UTF-8.
pub fn utf8<'e>(e: &'e git2::ConfigEntry<'_>) -> Option<&'e str> {
    raw(e).and_then(|b| std::str::from_utf8(b).ok())
}

/// The entry's value as text, with bytes that aren't UTF-8 replaced; None for a bare key.
pub fn lossy<'e>(e: &'e git2::ConfigEntry<'_>) -> Option<Cow<'e, str>> {
    raw(e).map(String::from_utf8_lossy)
}

/// The value of `key` (the entry `get_entry` picks, the same one `get_string` reads). A bare key
/// reads as "true", git's implicit true for keys that take a boolean or a word. Bytes that
/// aren't UTF-8 are replaced. None when unset or unreadable.
pub fn bool_or_word(cfg: &git2::Config, key: &str) -> Option<String> {
    let e = cfg.get_entry(key).ok()?;
    Some(lossy(&e).map_or_else(|| "true".to_owned(), Cow::into_owned))
}

#[cfg(test)]
mod tests;

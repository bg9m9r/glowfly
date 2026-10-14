//! Per-path settings read from gitattributes.

use git2::{AttrCheckFlags, AttrValue, Repository};
use std::path::Path;

/// The `linguist-language` value for `path` (repo-relative, '/'-separated), from the working
/// tree's attributes then the index's (also `.git/info/attributes` and `core.attributesFile`).
/// `None` when unset, set without a value, unspecified, empty, not UTF-8, or on any libgit2 error.
pub(crate) fn linguist_language(repo: &Repository, path: &str) -> Option<String> {
    let raw = repo.get_attr_bytes(Path::new(path), "linguist-language", AttrCheckFlags::FILE_THEN_INDEX).ok()?;
    match AttrValue::from_bytes(raw) {
        AttrValue::String(s) => Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    }
}

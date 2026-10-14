//! Per-path settings read from gitattributes.

use crate::error::{Error, Result};
use git2::{AttrCheckFlags, AttrValue, Repository};
use std::path::Path;

/// The `linguist-language` value for `path` (repo-relative, '/'-separated), from the working
/// tree's attributes then the index's (also `.git/info/attributes` and `core.attributesFile`).
/// `None` when unset, set without a value, unspecified, empty, not UTF-8, or on any libgit2 error.
pub(crate) fn linguist_language(repo: &Repository, path: &str) -> Option<String> {
    string_attr(repo, path, "linguist-language")
}

fn string_attr(repo: &Repository, path: &str, name: &str) -> Option<String> {
    let raw = repo.get_attr_bytes(Path::new(path), name, AttrCheckFlags::FILE_THEN_INDEX).ok()?;
    match AttrValue::from_bytes(raw) {
        AttrValue::String(s) => Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    }
}

/// The content conversion git applies to `path` between the working tree and the index that
/// libgit2 can't run: an external `filter` driver (Git LFS, git-crypt, ...) or a
/// `working-tree-encoding`. Shown as the attribute, e.g. `filter=lfs`.
pub(crate) fn content_conversion(repo: &Repository, path: &str) -> Option<String> {
    ["filter", "working-tree-encoding"].into_iter().find_map(|name| string_attr(repo, path, name).map(|v| format!("{name}={v}")))
}

/// Refuses a patch-based edit of `path` (by hunk or line) when git converts its content on the
/// way in or out: the patch would be built and applied to unconverted bytes, storing the wrong
/// content. Whole-file operations go through git and are fine.
pub(crate) fn refuse_converted(repo: &Repository, path: &str, what: &str) -> Result<()> {
    match content_conversion(repo, path) {
        Some(attr) => Err(Error::Unsupported(format!(
            "{path} has {attr} in .gitattributes, so git converts its content and {what} by hunk or line would store the wrong bytes; use the whole-file action instead"
        ))),
        None => Ok(()),
    }
}

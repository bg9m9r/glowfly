//! Per-path settings read from gitattributes, from the source the version shown calls for
//! (`attr_source.rs`): the working tree's and the index's files for working-tree content, the
//! index's for staged content, a commit's own files for a version in history. The checks before
//! editing working-tree content (`content_conversion`, `refuse_converted`, `lfs_filtered`) read
//! the working tree's.

use super::attr_source::AttrRepo;
use crate::error::{Error, Result};
use git2::{AttrCheckFlags, AttrValue, Blob, Repository};
use std::path::Path;

/// The `linguist-language` value for `path` (repo-relative, '/'-separated) from `attrs`'s source.
/// `None` when unset, set without a value, unspecified, empty, not UTF-8, or on any libgit2 error.
pub(crate) fn linguist_language(attrs: &AttrRepo, path: &str) -> Option<String> {
    string_value(attrs.attr(path, "linguist-language"))
}

/// Whether `path`'s content `blob` is binary as git's diff sees it, with `attrs`'s source:
/// `-diff` (or `binary`) makes it binary and `diff` text; a diff driver decides when it sets
/// `diff.<driver>.binary`; otherwise the content does (a NUL in its first 8,000 bytes).
pub(crate) fn is_binary(attrs: &AttrRepo, path: &str, blob: &Blob) -> bool {
    match attrs.attr(path, "diff") {
        AttrValue::False => true,
        AttrValue::True => false,
        AttrValue::String(driver) => {
            let set = attrs.config().ok().and_then(|c| c.get_bool(&format!("diff.{driver}.binary")).ok());
            set.unwrap_or_else(|| blob.is_binary())
        }
        _ => blob.is_binary(),
    }
}

/// Whether `path` has `filter=lfs` (the working tree's attributes, then the index's).
pub(super) fn lfs_filtered(repo: &Repository, path: &str) -> bool {
    workdir_attr(repo, path, "filter").as_deref() == Some("lfs")
}

/// A string attribute from the working tree's attributes, then the index's.
fn workdir_attr(repo: &Repository, path: &str, name: &str) -> Option<String> {
    parse_value(repo.get_attr_bytes(Path::new(path), name, AttrCheckFlags::FILE_THEN_INDEX).ok()?)
}

/// A string attribute's value, trimmed; `None` unless it is a non-empty UTF-8 string.
fn parse_value(raw: Option<&[u8]>) -> Option<String> {
    string_value(AttrValue::from_bytes(raw))
}

fn string_value(v: AttrValue) -> Option<String> {
    match v {
        AttrValue::String(s) => Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    }
}

/// The content conversion git applies to `path` between the working tree and the index that
/// libgit2 can't run: an external `filter` driver (Git LFS, git-crypt, ...) or a
/// `working-tree-encoding`. Shown as the attribute, e.g. `filter=lfs`.
pub(crate) fn content_conversion(repo: &Repository, path: &str) -> Option<String> {
    ["filter", "working-tree-encoding"].into_iter().find_map(|name| workdir_attr(repo, path, name).map(|v| format!("{name}={v}")))
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

#[cfg(test)]
#[path = "attrs_tests.rs"]
mod tests;

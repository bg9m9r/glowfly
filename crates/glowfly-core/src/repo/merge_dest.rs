//! What git's merge title ends with: " into <branch>" names the branch being merged into, unless
//! a `merge.suppressDest` glob covers it (main and master when the key is never set). Read at load
//! so the menus can hand git the title it would have written for a short branch name.

use git2::{Config, Repository};

/// The globs git uses when `merge.suppressDest` is never set.
const DEFAULT_GLOBS: [&str; 2] = ["main", "master"];

/// The branch git's merge title ends with (" into <it>") for merges into HEAD now: the checked-out
/// branch (`head_branch`), or "HEAD" when detached; None when merge.suppressDest covers it.
/// The globs are read in config order: an empty value clears the earlier ones, and with none set
/// they are main and master. Matching is git's wildmatch with `*` stopping at '/'.
pub(crate) fn merge_into(repo: &Repository, head_branch: Option<&str>) -> Option<String> {
    let cfg = repo.config().and_then(|mut c| c.snapshot()).ok();
    merge_into_with(cfg.as_ref(), head_branch)
}

/// [`merge_into`] read from `cfg` (None when the config can't be read: git's defaults).
fn merge_into_with(cfg: Option<&Config>, head_branch: Option<&str>) -> Option<String> {
    let dest = head_branch.unwrap_or("HEAD");
    let globs = cfg.and_then(globs).unwrap_or_else(|| DEFAULT_GLOBS.map(str::to_owned).to_vec());
    (!suppressed(&globs, dest)).then(|| dest.to_owned())
}

/// The suppressDest globs in effect, or None for git's defaults (never set).
fn globs(cfg: &Config) -> Option<Vec<String>> {
    let values = crate::repo::remotes::values(cfg, "merge.suppressdest");
    if values.is_empty() {
        return None;
    }
    let mut globs = Vec::new();
    for v in values {
        if v.is_empty() {
            globs.clear();
        } else {
            globs.push(v);
        }
    }
    Some(globs)
}

/// Whether any of `globs` matches the whole of `dest` (`*` never crosses '/').
fn suppressed(globs: &[String], dest: &str) -> bool {
    globs.iter().any(|g| crate::config_includes::wildmatch::matches(g.as_bytes(), dest.as_bytes(), false))
}

#[cfg(test)]
#[path = "merge_dest_tests.rs"]
mod tests;

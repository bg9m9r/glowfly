//! What a merge from the menus needs (`MergeNames`), read at load: the branches whose short name
//! git takes for another ref, and what git's merge title ends with (" into <branch>" names the
//! branch being merged into, unless a `merge.suppressDest` glob covers it: main and master when
//! the key is never set), so a shadowed branch can be merged by its full ref with git's title.

use crate::branch_ref::{BranchRef, MergeNames};
use crate::names::fold;
use git2::{Config, Repository};
use std::collections::HashSet;

/// The globs git uses when `merge.suppressDest` is never set.
const DEFAULT_GLOBS: [&str; 2] = ["main", "master"];

/// The prefixes git's short-name lookup tries, in order, after the name itself (which only an
/// all-caps name like ORIG_HEAD can match).
const LOOKUP: [&str; 4] = ["refs/", "refs/tags/", "refs/heads/", "refs/remotes/"];

/// The `MergeNames` for merges into HEAD now (`head_branch` checked out, None when detached),
/// with every one of `branches` checked for a shadowing ref.
pub(crate) fn merge_names(repo: &Repository, head_branch: Option<&str>, branches: &[BranchRef]) -> MergeNames {
    // Every ref name, read once (a lookup per candidate would stat files thousands of times) and
    // folded: on Windows and macOS git's lookup of refs/tags/x opens a loose refs/tags/X.
    let refs: HashSet<String> = repo.references().map(|mut r| r.names().filter_map(|n| n.ok().map(fold)).collect()).unwrap_or_default();
    let root_file = |name: &str| repo.path().join(name).is_file() || repo.commondir().join(name).is_file();
    let shadowed = branches.iter().filter(|b| shadowed(b, &refs, root_file)).map(BranchRef::full).collect();
    MergeNames { into: merge_into(repo, head_branch), shadowed }
}

/// Whether git, given `b`'s short name, picks something other than `b`: an object id spelled out
/// in full, an all-caps file in the git dir (`root_file`), or a ref in `refs` that comes before
/// `b`'s own in git's lookup order. `refs` holds folded names (`names::fold`), so one spelled with
/// other case or normalization counts too: loose ref files match that way on Windows and macOS,
/// and treating a branch as shadowed where they don't only costs it git's shortlog header.
fn shadowed(b: &BranchRef, refs: &HashSet<String>, root_file: impl Fn(&str) -> bool) -> bool {
    let short = b.shown();
    let full = b.full();
    let all_caps = short.bytes().all(|c| c.is_ascii_uppercase() || c == b'_');
    let object_id = matches!(short.len(), 40 | 64) && short.bytes().all(|c| c.is_ascii_hexdigit());
    object_id
        || (all_caps && root_file(short))
        || LOOKUP.iter().map(|p| format!("{p}{short}")).take_while(|c| *c != full).any(|c| refs.contains(&fold(&c)))
}

/// The branch git's merge title ends with (" into <it>") for merges into HEAD now: the checked-out
/// branch (`head_branch`), or "HEAD" when detached; None when merge.suppressDest covers it.
/// The globs are read in config order: an empty value clears the earlier ones, and with none set
/// they are main and master. Matching is git's wildmatch with `*` stopping at '/'.
fn merge_into(repo: &Repository, head_branch: Option<&str>) -> Option<String> {
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

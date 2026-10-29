//! One file's lines in one commit mapped onto another commit's version of it, following
//! renames; and the pull request form of that, which maps review threads' lines from the
//! service's head commit onto the local source tip.
//!
//! The changed regions come from a zero-context tree-to-tree diff, so each hunk is exactly one
//! changed region (git's `--unified=0` hunk headers). A line inside a region was changed or
//! removed and has no counterpart; a line after a region moves by the region's growth.

use crate::error::Result;
use git2::{Delta, DiffOptions, Oid, Patch, Repository};
use std::collections::HashMap;
use std::path::Path;

use super::{exact, pr_compare, rpath};

/// One changed region, as git's zero-context hunk header `@@ -old_start,old_lines
/// +new_start,new_lines @@` gives it. A side with no lines starts at the line the change sits
/// after (0 for the start of the file).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineChange {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

/// Where one file's lines are in the other commit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LineMap {
    /// The file's path in the other commit (a rename's new path); None when it is gone there
    /// or its path isn't UTF-8.
    pub path: Option<String>,
    /// The changed regions in order; None when the lines can't be compared (a binary file).
    pub changes: Option<Vec<LineChange>>,
}

impl LineMap {
    /// The file is at `path` with every line where it was.
    pub fn same(path: &str) -> LineMap {
        LineMap { path: Some(path.to_owned()), changes: Some(Vec::new()) }
    }

    /// The file isn't in the other commit.
    pub fn gone() -> LineMap {
        LineMap { path: None, changes: None }
    }

    /// Where line `n` (1-based) is in the other commit, or None when it was changed or
    /// removed, or the file can't be compared.
    pub fn line(&self, n: u32) -> Option<u32> {
        let changes = self.changes.as_ref().filter(|_| self.path.is_some() && n > 0)?;
        let mut shift: i64 = 0;
        for c in changes {
            if c.old_lines > 0 {
                if n < c.old_start {
                    break;
                }
                if n < c.old_start + c.old_lines {
                    return None;
                }
            } else if n <= c.old_start {
                break;
            }
            shift += i64::from(c.new_lines) - i64::from(c.old_lines);
        }
        u32::try_from(i64::from(n) + shift).ok().filter(|m| *m > 0)
    }
}

/// The threads' files mapped from the service's head onto the local source tip.
#[derive(Clone, PartialEq, Debug)]
pub enum PrLineMaps {
    /// The head commit isn't in this repository.
    HeadMissing,
    Mapped {
        /// Deleted-line numbers still hold: the head's merge base with the target is the local
        /// diff's base.
        old_kept: bool,
        /// Keyed by the head's path.
        files: HashMap<String, LineMap>,
    },
}

/// Maps `paths` (as commit `from` names them) onto commit `to`.
pub fn line_maps(workdir: &Path, from: Oid, to: Oid, paths: &[String]) -> Result<HashMap<String, LineMap>> {
    let repo = Repository::open(workdir)?;
    maps(&repo, from, to, paths)
}

/// Maps the threads' `paths` from the head the service listed (`head`, a commit id or prefix)
/// onto `local`, the source tip the files were loaded from. `target` is the target's tip and
/// `base` the local diff's base.
pub fn pr_line_maps(workdir: &Path, head: &str, local: Oid, target: Oid, base: Oid, paths: &[String]) -> Result<PrLineMaps> {
    let repo = Repository::open(workdir)?;
    let Some(h) = pr_compare::find_commit(&repo, head) else { return Ok(PrLineMaps::HeadMissing) };
    let old_kept = repo.merge_base(target, h).ok() == Some(base);
    let files = maps(&repo, h, local, paths)?;
    Ok(PrLineMaps::Mapped { old_kept, files })
}

fn maps(repo: &Repository, from: Oid, to: Oid, paths: &[String]) -> Result<HashMap<String, LineMap>> {
    let mut o = DiffOptions::new();
    o.context_lines(0).interhunk_lines(0);
    // A whole-tree diff: a pathspec would hide a rename's other side, and identical subtrees
    // are skipped by id anyway.
    // Binary files as the comparison's diff sees them: by `to`'s gitattributes.
    let wanted: Vec<&str> = paths.iter().map(String::as_str).collect();
    let attrs = super::attr_source::attr_repo(repo, super::attr_source::AttrSource::Commit(to), &wanted);
    let diff = super::diff::trees_diff(&attrs, from, to, Some(&mut o))?;
    let from_tree = repo.find_commit(from)?.tree()?;
    let mut out = HashMap::new();
    for p in paths {
        let found =
            diff.deltas().enumerate().find(|(_, d)| rpath::is(&d.old_file(), p) && !matches!(d.status(), Delta::Added | Delta::Untracked));
        let map = match found {
            None if exact::tree_entry(repo, &from_tree, p.as_bytes()).is_some() => LineMap::same(p),
            None => LineMap::gone(),
            Some((_, d)) if d.status() == Delta::Deleted => LineMap::gone(),
            Some((i, d)) => {
                let path = rpath::text(d.new_file().path_bytes().unwrap_or_default());
                let patch = Patch::from_diff(&diff, i)?.filter(|p| !p.delta().flags().is_binary());
                let changes = match patch {
                    // The same content (only the mode changed) has no patch to read.
                    None if d.old_file().id() == d.new_file().id() => Some(Vec::new()),
                    None => None,
                    Some(patch) => Some(
                        (0..patch.num_hunks())
                            .map(|h| {
                                patch.hunk(h).map(|(hunk, _)| LineChange {
                                    old_start: hunk.old_start(),
                                    old_lines: hunk.old_lines(),
                                    new_start: hunk.new_start(),
                                    new_lines: hunk.new_lines(),
                                })
                            })
                            .collect::<std::result::Result<Vec<_>, _>>()?,
                    ),
                };
                LineMap { path, changes }
            }
        };
        out.insert(p.clone(), map);
    }
    Ok(out)
}

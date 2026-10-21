//! Which commits changed a path matching the search's path terms (`path_query`), for the
//! graph search.

use crate::error::Result;
use crate::path_query::{PathTerm, any_matches};
use git2::{DiffDelta, DiffOptions, Oid, Repository};
use std::path::Path;

/// How far [`commits_touching`] got and what it found.
#[derive(Debug, Default, PartialEq)]
pub struct Touching {
    /// How many of the commits asked about were looked at (from the first).
    pub gone_through: usize,
    /// Those that touch a term, in the order asked.
    pub touching: Vec<Oid>,
}

/// Which of `oids` (in order) changed a path matching one of `terms` against their first parent:
/// root commits against the empty tree, renames not followed (the old and new paths are both
/// deltas). Stops before an oid (never before the first) once `stop()` is true. A commit that
/// can't be read (a missing object in a shallow clone) counts as gone through, not touching.
pub fn commits_touching(workdir: &Path, oids: &[Oid], terms: &[PathTerm], stop: &dyn Fn() -> bool) -> Result<Touching> {
    let repo = Repository::open(workdir)?;
    let mut opts = DiffOptions::new();
    opts.skip_binary_check(true);
    // When every term names a place from the root in ASCII, libgit2 may skip the trees outside
    // them. The pathspecs are literal (PLATFORMS repo-path rule 4: no glob-reading pathspecs) and
    // only prune; the check of each delta below decides.
    let prefixes: Option<Vec<&str>> = terms.iter().map(PathTerm::literal_prefix).collect();
    if let Some(prefixes) = prefixes.filter(|p| !p.is_empty()) {
        for p in prefixes {
            opts.pathspec(p);
        }
        opts.disable_pathspec_match(true).ignore_case(true);
    }
    let mut out = Touching::default();
    for (i, &oid) in oids.iter().enumerate() {
        if i > 0 && stop() {
            break;
        }
        out.gone_through += 1;
        if touches(&repo, oid, terms, &mut opts).unwrap_or(false) {
            out.touching.push(oid);
        }
    }
    Ok(out)
}

/// Whether commit `oid` changed a path matching `terms` against its first parent.
fn touches(repo: &Repository, oid: Oid, terms: &[PathTerm], opts: &mut DiffOptions) -> Result<bool> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent = if commit.parent_count() > 0 { Some(commit.parent(0)?.tree()?) } else { None };
    // No find_similar: renames stay a delete and an add, and no blob is read.
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&tree), Some(opts))?;
    Ok(diff.deltas().any(|d| delta_matches(&d, terms)))
}

/// Whether the delta's old or new path matches (the same path unless it is a rename or copy).
fn delta_matches(d: &DiffDelta<'_>, terms: &[PathTerm]) -> bool {
    let side = |p: Option<&[u8]>| p.is_some_and(|p| any_matches(terms, p));
    side(d.old_file().path_bytes()) || side(d.new_file().path_bytes())
}

//! Narrowing one file's commit or comparison diff to that file: a literal pathspec of its path,
//! plus the other side's for a rename or a copy, instead of the whole tree. A comparison with
//! the working tree then stats two paths rather than every file (and walks no untracked folder
//! off their way), a tree diff skips every other folder, and rename detection compares only the
//! pair.
//!
//! Only a narrowed diff that must show the file as the whole one does is used:
//! - a modified or type-changed file: rename detection never touches it (nor its patch);
//! - a file whose row in the list loaded with the whole diff (`load_listed_diff`) says the same
//!   as the narrowed diff: its status, and its rename or copy source, which is the only path it
//!   can pair with (an added or deleted row names none, so it paired with nothing).
//!
//! Anything else (an added or deleted file with no row to go by, a row the working tree no
//! longer matches, a path the literal pathspec misses) reads the whole diff, as before.

use super::diff::{delta_entry, finds_renames, plain_target_diff, renamed, target_diff};
use super::rpath::{find_delta, text, touches};
use super::*;
use crate::error::Result;
use git2::{Delta, Diff, DiffDelta, Repository};
use std::path::Path;

/// `target`'s diff for viewing its file, renames detected (`partial::find_renames`), narrowed
/// to the file when that shows it as the whole diff does (see the module doc).
pub(super) fn file_diff<'r>(
    repo: &'r Repository,
    workdir: &Path,
    target: &DiffTarget,
    opts: LoadDiffOptions,
    listed: Option<&FileEntry>,
) -> Result<Diff<'r>> {
    let path = target.path();
    if !finds_renames(target) {
        // The staged and unstaged diffs are their own path's already.
        return plain_target_diff(repo, target, opts, None, None);
    }
    let listed = listed.filter(|f| f.path == path && f.hazard.is_none());
    if let Some(diff) = narrowed(repo, workdir, target, opts, listed)? {
        return Ok(diff);
    }
    let mut diff = plain_target_diff(repo, target, opts, None, None)?;
    crate::partial::find_renames(repo, workdir, target, &mut diff, |d| touches(d, path))?;
    Ok(diff)
}

/// The diff of `target`'s path and `listed`'s rename or copy source, when it shows the file as
/// the whole diff would; `None` when that can't be told.
fn narrowed<'r>(
    repo: &'r Repository,
    workdir: &Path,
    target: &DiffTarget,
    opts: LoadDiffOptions,
    listed: Option<&FileEntry>,
) -> Result<Option<Diff<'r>>> {
    let path = target.path();
    let partner = listed.and_then(|f| f.renamed_from.as_deref()).filter(|p| *p != path);
    let scope: Vec<&str> = std::iter::once(path).chain(partner).collect();
    let mut diff = plain_target_diff(repo, target, opts, None, Some(&scope))?;
    if partner.is_some() {
        crate::partial::find_renames(repo, workdir, target, &mut diff, |d| touches(d, path))?;
    }
    let reversed = matches!(target, DiffTarget::FromWorkdir(..));
    let Some(d) = find_delta(&diff, path) else { return Ok(None) };
    let shown = delta_entry(&d, reversed);
    let same = match listed {
        Some(f) => shown.path == path && shown.status == f.status && shown.renamed_from == f.renamed_from,
        None => matches!(d.status(), Delta::Modified | Delta::Typechange),
    };
    Ok(same.then_some(diff))
}

/// `target`'s diff with `context` lines for the whole-file view, `first` being the diff its
/// hunks came from (`file_diff`): narrowed to the paths of `first`'s delta for the file, renames
/// detected only when that is a rename or a copy. The whole diff is read when the narrowed one
/// shows the file differently.
pub(super) fn whole_diff<'r>(
    repo: &'r Repository,
    target: &DiffTarget,
    opts: LoadDiffOptions,
    first: &Diff<'_>,
    context: u32,
) -> Result<Diff<'r>> {
    let path = target.path();
    let Some(was) = find_delta(first, path).filter(|_| finds_renames(target)) else {
        return target_diff(repo, target, opts, Some(context));
    };
    let Some((old, new)) = sides(&was) else { return target_diff(repo, target, opts, Some(context)) };
    let scope: Vec<&str> = if old == new { vec![&new] } else { vec![&new, &old] };
    let mut diff = plain_target_diff(repo, target, opts, Some(context), Some(&scope))?;
    if matches!(was.status(), Delta::Renamed | Delta::Copied) {
        diff = renamed(repo, diff)?;
    }
    let same = find_delta(&diff, path).is_some_and(|d| d.status() == was.status() && sides(&d) == sides(&was));
    if same { Ok(diff) } else { target_diff(repo, target, opts, Some(context)) }
}

/// A delta's old and new paths, when both are UTF-8.
fn sides(d: &DiffDelta<'_>) -> Option<(String, String)> {
    Some((text(d.old_file().path_bytes()?)?, text(d.new_file().path_bytes()?)?))
}

#[cfg(test)]
#[path = "diff_scope_tests.rs"]
mod tests;

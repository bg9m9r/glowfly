//! Loading one file's diff for the viewer (`load_diff_for_view`, `load_listed_diff`): its hunks,
//! the whole file when asked, the old file's text for highlighting and the path's language. The
//! diffs it reads are narrowed to that file where they can be (`diff_scope`).

use super::diff::{file_patch, hunk_of};
use super::*;
use crate::error::Result;
use crate::highlight::{self, OldText};
use git2::{Patch, Repository};
use std::path::Path;
use std::sync::Arc;

/// Context for the whole-file load: more lines than any real file has, so libgit2 returns one
/// hunk covering the whole file. Not u32::MAX: xdiff keeps the context in a C long (32 bits on
/// Windows) and doubles it.
const WHOLE_FILE_CONTEXT: u32 = 1 << 24;

/// A diff loaded for the viewer, with what highlighting it needs.
#[derive(Debug)]
pub struct ViewDiff {
    pub diff: FileDiff,
    /// The old file's text, for highlighting the hunks in context (see `old_text`).
    pub old: Option<Arc<OldText>>,
    /// The path's `linguist-language` gitattribute, raw (resolved by `highlight::SyntaxHint`).
    pub language: Option<String>,
    /// The two sides when either is a Git LFS pointer (`lfs_marks::diff_change`).
    pub lfs: Option<crate::lfs::pointer::LfsChange>,
    /// For an unstaged diff of one hunk: its lines' exact bytes and the working file's own
    /// bytes, hashed (`index::raw_id`), which the hunk's text (decoded, line ends trimmed) and git's
    /// converted content (`core.autocrlf`) can't tell apart.
    pub raw: Option<Oid>,
}

/// `load_diff_with` plus the old file's text, for highlighting the hunks in context
/// (`highlight::highlight_in_context`), and the path's `linguist-language` gitattribute (`None`
/// for binary files). Attributes, binary detection's included, come from the version shown
/// (`AttrSource::of_target`): the commit's own `.gitattributes` files for commit and comparison
/// diffs, the index's for staged changes, the working tree's then the index's for the rest. The
/// text is `None` for added, untracked or binary files, a whole-file load (its one hunk is the
/// whole file), an old blob over `highlight::OLD_TEXT_MAX_BYTES`, or one that doesn't match
/// the hunks.
pub fn load_diff_for_view(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions) -> Result<ViewDiff> {
    load(workdir, target, opts, true, None)
}

/// `load_diff_for_view` for a file opened from a list loaded with the whole commit or
/// comparison (`compare`, `commit_detail`, ...): `listed` is the file's row there, whose status
/// and rename or copy source (`FileEntry::renamed_from`) let the diff read only that file and
/// its partner. The result is the same as `load_diff_for_view`'s; a row that no longer matches
/// what git reads (the working tree changed since) makes the whole comparison be read.
pub fn load_listed_diff(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions, listed: Option<&FileEntry>) -> Result<ViewDiff> {
    load(workdir, target, opts, true, listed)
}

pub(super) fn load(
    workdir: &Path,
    target: &DiffTarget,
    opts: LoadDiffOptions,
    with_old: bool,
    listed: Option<&FileEntry>,
) -> Result<ViewDiff> {
    let pooled = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir)?;
    // The diff runs on a handle reading the shown version's gitattributes, so libgit2's binary
    // detection follows them too.
    let repo = super::attr_source::for_target(&pooled, target);
    let path = target.path();
    // Renames are detected in it (a partial clone may not have the blobs yet; libgit2 can't
    // fetch them).
    let diff = super::diff_scope::file_diff(&repo, workdir, target, opts, listed)?;
    if let Some(delta) = super::rpath::find_delta(&diff, path) {
        crate::partial::require(&repo, workdir, path, crate::partial::delta_blobs(target, &delta))?;
    }
    let lfs = with_old.then(|| super::lfs_marks::diff_change(&repo, target, &diff)).flatten();
    let mut out = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    let Some(patch) = file_patch(&diff, path)? else {
        out.binary = true;
        return Ok(ViewDiff { diff: out, old: None, language: None, lfs, raw: None });
    };
    out.binary = patch.delta().flags().is_binary();
    for h in 0..patch.num_hunks() {
        out.hunks.push(hunk_of(&patch, h)?);
    }
    if opts.whole_file && !out.binary && !out.hunks.is_empty() {
        let diff = super::diff_scope::whole_diff(&repo, target, opts, &diff, WHOLE_FILE_CONTEXT)?;
        if let Some(patch) = file_patch(&diff, path)?
            && patch.num_hunks() == 1
        {
            let whole = hunk_of(&patch, 0)?;
            out.whole = crate::whole_file::WholeFile::new(&out.hunks, whole).map(Box::new);
        }
    }
    // `patch` is still the first load's (the whole-file block's is scoped to it).
    let old = if with_old && !out.binary && out.whole.is_none() { old_text(&repo, &patch, &out.hunks) } else { None };
    let language = if with_old && !out.binary { super::attrs::linguist_language(&repo, path) } else { None };
    Ok(ViewDiff { diff: out, old, language, lfs, raw: super::index::shown_raw_id(workdir, target, path, &patch)? })
}

/// The text of the delta's old side (the index for unstaged changes, HEAD for staged ones, the
/// first parent or `from` for commits, the commit for a working-tree comparison, at the old
/// path for renames), checked against `hunks`.
fn old_text(repo: &Repository, patch: &Patch<'_>, hunks: &[Hunk]) -> Option<Arc<OldText>> {
    let id = patch.delta().old_file().id();
    if id.is_zero() || hunks.is_empty() {
        return None;
    }
    let blob = repo.find_blob(id).ok()?;
    if blob.is_binary() || blob.size() > highlight::OLD_TEXT_MAX_BYTES {
        return None;
    }
    OldText::new(blob.content(), hunks).map(Arc::new)
}

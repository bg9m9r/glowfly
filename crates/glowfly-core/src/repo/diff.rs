//! Comparisons between two commits, file diffs and per-commit line counts.

use super::*;
use crate::error::{Error, Result};
use git2::{Delta, Diff, DiffDelta, DiffOptions, Oid, Patch, Repository, Tree};
use std::collections::HashSet;
use std::path::Path;

/// Lines added and removed, and files changed, for each commit (against its first parent).
pub fn commit_stats(workdir: &Path, oids: &[Oid]) -> Vec<(Oid, usize, usize, usize)> {
    commit_stats_until(workdir, oids, &|| false).0
}

/// `commit_stats`, stopping before the next commit once `stop` returns true. Also returns how
/// many of `oids` were gone through (those that failed included).
pub fn commit_stats_until(workdir: &Path, oids: &[Oid], stop: &dyn Fn() -> bool) -> (Vec<(Oid, usize, usize, usize)>, usize) {
    let Ok(repo) = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir) else { return (Vec::new(), oids.len()) };
    // One handle for every commit, pointed at each one's gitattributes in turn, so binary files
    // (which count no lines) are those the commit's own attributes say.
    let attrs = super::attr_source::bare_handle(&repo);
    let mut out = Vec::new();
    for (i, &oid) in oids.iter().enumerate() {
        if i > 0 && stop() {
            return (out, i);
        }
        let paths = commit_paths(&repo, oid);
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let on = match &attrs {
            Some(h) if super::attr_source::point_at(h, oid, &paths) => h,
            _ => &*repo,
        };
        if let Some(stats) = commit_diff(on, oid, None).ok().and_then(|d| d.stats().ok()) {
            out.push((oid, stats.files_changed(), stats.insertions(), stats.deletions()));
        }
    }
    (out, oids.len())
}

pub(super) fn delta_char(d: Delta) -> char {
    match d {
        Delta::Added | Delta::Untracked | Delta::Copied => 'A',
        Delta::Deleted => 'D',
        Delta::Renamed => 'R',
        Delta::Typechange => 'T',
        Delta::Conflicted => '!',
        _ => 'M',
    }
}

pub(super) fn head_tree(repo: &Repository) -> Option<Tree<'_>> {
    repo.head().ok().and_then(|h| h.peel_to_tree().ok())
}

pub(super) fn commit_diff<'r>(repo: &'r Repository, oid: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    renamed(repo, commit_plain(repo, oid, opts)?)
}

/// Both sides' paths of commit `oid`'s changes, whose gitattributes to read. Empty on error.
pub(super) fn commit_paths(repo: &Repository, oid: Oid) -> Vec<String> {
    commit_plain(repo, oid, None).map(|d| super::attr_source::delta_paths(&d)).unwrap_or_default()
}

/// `commit_diff` without rename detection.
fn commit_plain<'r>(repo: &'r Repository, oid: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let parent_tree = if c.parent_count() > 0 { Some(c.parent(0)?.tree()?) } else { None };
    Ok(repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), opts)?)
}

/// `diff` with renames detected. Detection compares the deleted and added files' contents, which
/// a partial clone may not have: there the deletes and adds stay as they are (the diff pane asks
/// for those contents instead, `partial::find_renames`).
pub(super) fn renamed<'r>(repo: &Repository, mut diff: Diff<'r>) -> Result<Diff<'r>> {
    match diff.find_similar(None) {
        Err(e) if e.code() == git2::ErrorCode::NotFound && crate::partial::looks_partial(repo) => Ok(diff),
        found => found.map(|()| diff).map_err(Into::into),
    }
}

/// One end of a comparison: commit `oid`'s tree, or nothing for the empty tree.
fn side_tree(repo: &Repository, oid: Oid) -> Result<Option<Tree<'_>>> {
    if is_empty_tree(oid) {
        return Ok(None);
    }
    Ok(Some(repo.find_commit(oid)?.tree()?))
}

/// The tree-to-tree diff from commit `from` to commit `to` (either may be the empty tree), with
/// renames detected.
pub(super) fn trees_diff<'r>(repo: &'r Repository, from: Oid, to: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    renamed(repo, trees_plain(repo, from, to, opts)?)
}

fn trees_plain<'r>(repo: &'r Repository, from: Oid, to: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let (from_tree, to_tree) = (side_tree(repo, from)?, side_tree(repo, to)?);
    Ok(repo.diff_tree_to_tree(from_tree.as_ref(), to_tree.as_ref(), opts)?)
}

/// The diff from commit `oid`'s tree to the working tree (through the index, so staged and
/// unstaged changes both count), untracked files included and renames detected; `reversed`
/// runs it from the working tree to the commit. Ignored files are left out.
fn workdir_diff<'r>(repo: &'r Repository, oid: Oid, reversed: bool, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    renamed(repo, workdir_plain(repo, oid, reversed, opts)?)
}

fn workdir_plain<'r>(repo: &'r Repository, oid: Oid, reversed: bool, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let tree = repo.find_commit(oid)?.tree()?;
    let mut fresh = DiffOptions::new();
    let o = opts.unwrap_or(&mut fresh);
    o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true).reverse(reversed);
    Ok(repo.diff_tree_to_workdir_with_index(Some(&tree), Some(o))?)
}

/// One entry per delta (`delta_entry`).
pub(super) fn file_entries(diff: &Diff, reversed: bool) -> Vec<FileEntry> {
    diff.deltas().map(|d| delta_entry(&d, reversed)).collect()
}

/// A delta's row, named by its new path (the old one for a deletion), with a rename's or a
/// copy's old path in `renamed_from`. In a `reversed` working-tree diff an untracked file is on
/// the old side only, so it is 'D'.
pub(super) fn delta_entry(d: &DiffDelta<'_>, reversed: bool) -> FileEntry {
    let status = if reversed && d.status() == Delta::Untracked { 'D' } else { delta_char(d.status()) };
    let mut row = super::rpath::entry(super::rpath::delta_bytes(d), status);
    if matches!(d.status(), Delta::Renamed | Delta::Copied) {
        row.renamed_from = d.old_file().path_bytes().and_then(super::rpath::text);
    }
    row
}

/// The files that differ from commit `from` to commit `to`: a file only in `to` is 'A', only in
/// `from` is 'D', and a rename is 'R' under its new path. In libgit2 path order.
pub fn compare_files(workdir: &Path, from: Oid, to: Oid) -> Result<Vec<FileEntry>> {
    let repo = Repository::open(workdir)?;
    Ok(file_entries(&trees_diff(&repo, from, to, None)?, false))
}

/// The files that differ from commit `from` to commit `to`, as `compare_files` lists them.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Comparison {
    pub files: Vec<FileEntry>,
    /// Paths of `files` missing from the working tree when loaded (deleted, renamed or never
    /// checked out). Views use it to disable Open / Show in folder.
    pub absent: HashSet<String>,
}

impl Comparison {
    /// Whether `path` was in the working tree when the comparison was loaded.
    pub fn in_workdir(&self, path: &str) -> bool {
        !self.absent.contains(path)
    }
}

/// `compare_files` plus which of the files are missing from the working tree.
pub fn compare(workdir: &Path, from: Oid, to: Oid) -> Result<Comparison> {
    comparison(&Repository::open(workdir)?, from, to)
}

fn comparison(repo: &Repository, from: Oid, to: Oid) -> Result<Comparison> {
    let diff = trees_diff(repo, from, to, None)?;
    let mut files = file_entries(&diff, false);
    super::lfs_marks::mark_delta_files(repo, &diff, &mut files, false);
    let absent = absent_paths(repo, &files);
    Ok(Comparison { files, absent })
}

/// The files that differ from commit `oid` to the working tree (staged and unstaged changes,
/// untracked files as 'A', renames as 'R' under the new path), plus which are missing from it.
pub fn compare_workdir(workdir: &Path, oid: Oid) -> Result<Comparison> {
    workdir_comparison(workdir, oid, false)
}

/// The files that differ from the working tree to commit `oid`: `compare_workdir` the other way
/// round, so untracked and newly staged files are 'D', files deleted from the working tree 'A',
/// and a rename is listed under the commit's name.
pub fn compare_from_workdir(workdir: &Path, oid: Oid) -> Result<Comparison> {
    workdir_comparison(workdir, oid, true)
}

fn workdir_comparison(workdir: &Path, oid: Oid, reversed: bool) -> Result<Comparison> {
    let repo = Repository::open(workdir)?;
    let diff = workdir_diff(&repo, oid, reversed, None)?;
    let mut files = file_entries(&diff, reversed);
    if reversed {
        super::lfs_marks::mark_reversed_workdir_files(&repo, &diff, &mut files);
    } else {
        super::lfs_marks::mark_delta_files(&repo, &diff, &mut files, true);
    }
    let absent = absent_paths(&repo, &files);
    Ok(Comparison { files, absent })
}

/// What a branch brings to its target: the files that differ from the merge base of `target`
/// and `source` to `source`.
#[derive(Clone, PartialEq, Debug)]
pub struct BranchChanges {
    pub base: Oid,
    pub comparison: Comparison,
}

/// The change `source` brings to `target`, as the comparison from their merge base to
/// `source`. `None` when the two share no history (or none that is in this repository). A
/// commit missing from the repository is an error.
pub fn branch_changes(workdir: &Path, target: Oid, source: Oid) -> Result<Option<BranchChanges>> {
    let repo = Repository::open(workdir)?;
    repo.find_commit(target)?;
    repo.find_commit(source)?;
    let base = match repo.merge_base(target, source) {
        Ok(base) => base,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    Ok(Some(BranchChanges { base, comparison: comparison(&repo, base, source)? }))
}

/// The paths of `files` with nothing at them in the working tree (all of them in a bare
/// repository, and every name that isn't UTF-8). One stat per file, so views can tell which
/// ones can be opened.
pub(super) fn absent_paths(repo: &Repository, files: &[FileEntry]) -> HashSet<String> {
    files.iter().filter(|f| f.hazard.is_some() || !on_disk(repo, &f.path)).map(|f| f.path.clone()).collect()
}

/// Whether something is at `path` in the working tree (never in a bare repository).
pub(super) fn on_disk(repo: &Repository, path: &str) -> bool {
    repo.workdir().is_some_and(|root| root.join(path).symlink_metadata().is_ok())
}

pub(super) fn path_opts(path: &str) -> DiffOptions {
    let mut o = DiffOptions::new();
    o.pathspec(path).disable_pathspec_match(true);
    o
}

/// Applies the viewer's whitespace filter to `o` (git `-b --ignore-space-at-eol`).
fn whitespace_opts(o: &mut DiffOptions, v: LoadDiffOptions) -> &mut DiffOptions {
    o.ignore_whitespace_change(v.ignore_whitespace).ignore_whitespace_eol(v.ignore_whitespace)
}

pub(super) fn workdir_opts(path: &str) -> DiffOptions {
    let mut o = path_opts(path);
    o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
    o
}

/// One file's hunks, with every change shown. Git only: `highlight` and `intraline` are left
/// empty for the caller to fill in with `highlight::highlight_diff` and `intraline::intraline`.
pub fn load_diff(workdir: &Path, target: &DiffTarget) -> Result<FileDiff> {
    load_diff_with(workdir, target, LoadDiffOptions::default())
}

/// `load_diff` with options. With whitespace ignored, a modified file whose changes are all
/// whitespace comes back with `hunks` empty rather than an error: the delta stays (deltas compare
/// blob ids) and only the patch loses its hunks. Hunk indexes from such a diff don't match
/// `apply_hunk` or `apply_lines`, which always work on the unfiltered diff: use `apply_shown`,
/// or `revert` with `RevertRequest::ignore_whitespace`, which apply only the changes shown.
///
/// With `whole_file`, the file is loaded a second time with unlimited context into
/// `FileDiff::whole`. That load uses the same whitespace options, so its changed lines are the
/// same edit script as the normal hunks (xdiff's context doesn't change which lines are
/// changed), which is what lets `WholeFile::new` map each hunk onto it.
pub fn load_diff_with(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions) -> Result<FileDiff> {
    super::diff_view::load(workdir, target, opts, false, None).map(|v| v.diff)
}

/// The diff behind `target`, with `context` lines around each change when set (and hunks
/// never merged because of nearby changes, so one hunk spans the file).
pub(super) fn target_diff<'r>(repo: &'r Repository, target: &DiffTarget, opts: LoadDiffOptions, context: Option<u32>) -> Result<Diff<'r>> {
    let diff = plain_target_diff(repo, target, opts, context, None)?;
    if finds_renames(target) { renamed(repo, diff) } else { Ok(diff) }
}

/// Whether `target`'s diff detects renames (the staged and unstaged ones are one path's).
pub(super) fn finds_renames(target: &DiffTarget) -> bool {
    !matches!(target, DiffTarget::Unstaged(_) | DiffTarget::Staged(_))
}

/// `target_diff` without rename detection. With `scope`, a commit's or a comparison's diff covers
/// only those paths (literally: no globbing, and a folder's files are under it); the staged and
/// unstaged diffs are always only their own path's.
pub(super) fn plain_target_diff<'r>(
    repo: &'r Repository,
    target: &DiffTarget,
    opts: LoadDiffOptions,
    context: Option<u32>,
    scope: Option<&[&str]>,
) -> Result<Diff<'r>> {
    let mut o = match target {
        DiffTarget::Commit(..) | DiffTarget::Between(..) | DiffTarget::ToWorkdir(..) | DiffTarget::FromWorkdir(..) => {
            let mut o = DiffOptions::new();
            for p in scope.unwrap_or_default() {
                o.pathspec(*p).disable_pathspec_match(true);
            }
            o
        }
        DiffTarget::Unstaged(p) => workdir_opts(p),
        DiffTarget::Staged(p) => path_opts(p),
    };
    whitespace_opts(&mut o, opts);
    if let Some(n) = context {
        o.context_lines(n).interhunk_lines(0);
    }
    Ok(match target {
        DiffTarget::Commit(oid, _) => commit_plain(repo, *oid, Some(&mut o))?,
        DiffTarget::Between(from, to, _) => trees_plain(repo, *from, *to, Some(&mut o))?,
        DiffTarget::ToWorkdir(oid, _) => workdir_plain(repo, *oid, false, Some(&mut o))?,
        DiffTarget::FromWorkdir(oid, _) => workdir_plain(repo, *oid, true, Some(&mut o))?,
        DiffTarget::Unstaged(_) => repo.diff_index_to_workdir(None, Some(&mut o))?,
        DiffTarget::Staged(_) => repo.diff_tree_to_index(head_tree(repo).as_ref(), None, Some(&mut o))?,
    })
}

/// Hunk `h` of `patch` as the viewer shows it.
pub(super) fn hunk_of(patch: &Patch<'_>, h: usize) -> Result<Hunk> {
    let (hunk, count) = patch.hunk(h)?;
    let mut lines = Vec::with_capacity(count);
    for l in 0..count {
        let line = patch.line_in_hunk(h, l)?;
        lines.push(DiffLine {
            origin: line.origin(),
            old: line.old_lineno(),
            new: line.new_lineno(),
            text: String::from_utf8_lossy(line.content()).trim_end_matches(['\n', '\r']).to_owned(),
        });
    }
    Ok(Hunk { header: String::from_utf8_lossy(hunk.header()).trim_end().to_owned(), lines })
}

/// The patch for `path` in `diff`: `NotFound` when the file has no changes, `None` when it is
/// binary. Shared by `load_diff` and line staging, so both see the same hunks and lines.
pub(super) fn file_patch<'b>(diff: &Diff<'b>, path: &str) -> Result<Option<Patch<'b>>> {
    let idx = super::rpath::delta_of(diff, path).ok_or_else(|| Error::NotFound(format!("no changes in {path}")))?;
    Ok(Patch::from_diff(diff, idx)?)
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod compare_tests;
#[cfg(test)]
#[path = "compare_workdir_tests.rs"]
mod compare_workdir_tests;

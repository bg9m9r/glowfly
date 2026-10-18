//! Commit details, comparisons between two commits, file diffs and per-commit line counts.

use super::*;
use crate::error::{Error, Result};
use crate::highlight::{self, OldText};
use git2::{Delta, Diff, DiffOptions, Oid, Patch, Repository, Tree};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

/// Lines added and removed, and files changed, for each commit (against its first parent).
pub fn commit_stats(workdir: &Path, oids: &[Oid]) -> Vec<(Oid, usize, usize, usize)> {
    commit_stats_until(workdir, oids, &|| false).0
}

/// `commit_stats`, stopping before the next commit once `stop` returns true. Also returns how
/// many of `oids` were gone through (those that failed included).
pub fn commit_stats_until(workdir: &Path, oids: &[Oid], stop: &dyn Fn() -> bool) -> (Vec<(Oid, usize, usize, usize)>, usize) {
    let Ok(repo) = Repository::open(workdir) else { return (Vec::new(), oids.len()) };
    let mut out = Vec::new();
    for (i, &oid) in oids.iter().enumerate() {
        if i > 0 && stop() {
            return (out, i);
        }
        if let Some(stats) = commit_diff(&repo, oid, None).ok().and_then(|d| d.stats().ok()) {
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

fn commit_diff<'r>(repo: &'r Repository, oid: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let parent_tree = if c.parent_count() > 0 { Some(c.parent(0)?.tree()?) } else { None };
    let mut diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), opts)?;
    diff.find_similar(None)?;
    Ok(diff)
}

/// The tree-to-tree diff from commit `from` to commit `to`, with renames detected.
fn trees_diff<'r>(repo: &'r Repository, from: Oid, to: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let from_tree = repo.find_commit(from)?.tree()?;
    let to_tree = repo.find_commit(to)?.tree()?;
    let mut diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), opts)?;
    diff.find_similar(None)?;
    Ok(diff)
}

/// The diff from commit `oid`'s tree to the working tree (through the index, so staged and
/// unstaged changes both count), untracked files included and renames detected. Ignored files
/// are left out.
fn workdir_diff<'r>(repo: &'r Repository, oid: Oid, opts: Option<&mut DiffOptions>) -> Result<Diff<'r>> {
    let tree = repo.find_commit(oid)?.tree()?;
    let mut fresh = DiffOptions::new();
    let o = opts.unwrap_or(&mut fresh);
    o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
    let mut diff = repo.diff_tree_to_workdir_with_index(Some(&tree), Some(o))?;
    diff.find_similar(None)?;
    Ok(diff)
}

/// One entry per delta, named by its new path (the old one for a deletion).
fn file_entries(diff: &Diff) -> Vec<FileEntry> {
    diff.deltas().map(|d| super::rpath::entry(super::rpath::delta_bytes(&d), delta_char(d.status()))).collect()
}

/// The files that differ from commit `from` to commit `to`: a file only in `to` is 'A', only in
/// `from` is 'D', and a rename is 'R' under its new path. In libgit2 path order.
pub fn compare_files(workdir: &Path, from: Oid, to: Oid) -> Result<Vec<FileEntry>> {
    let repo = Repository::open(workdir)?;
    Ok(file_entries(&trees_diff(&repo, from, to, None)?))
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
    let mut files = file_entries(&diff);
    super::lfs_marks::mark_delta_files(repo, &diff, &mut files, false);
    let absent = absent_paths(repo, &files);
    Ok(Comparison { files, absent })
}

/// The files that differ from commit `oid` to the working tree (staged and unstaged changes,
/// untracked files as 'A', renames as 'R' under the new path), plus which are missing from it.
pub fn compare_workdir(workdir: &Path, oid: Oid) -> Result<Comparison> {
    let repo = Repository::open(workdir)?;
    let diff = workdir_diff(&repo, oid, None)?;
    let mut files = file_entries(&diff);
    super::lfs_marks::mark_delta_files(&repo, &diff, &mut files, true);
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
fn absent_paths(repo: &Repository, files: &[FileEntry]) -> HashSet<String> {
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

pub fn commit_detail(workdir: &Path, oid: Oid) -> Result<CommitDetail> {
    let repo = Repository::open(workdir)?;
    let c = repo.find_commit(oid)?;
    let diff = commit_diff(&repo, oid, None)?;
    let mut files = file_entries(&diff);
    super::lfs_marks::mark_delta_files(&repo, &diff, &mut files, false);
    let absent = absent_paths(&repo, &files);
    Ok(CommitDetail {
        oid,
        message: c.message().unwrap_or_default().trim_end().to_owned(),
        author: c.author().name().unwrap_or_default().to_owned(),
        email: c.author().email().unwrap_or_default().to_owned(),
        committer: c.committer().name().unwrap_or_default().to_owned(),
        time: c.time().seconds(),
        parents: c.parent_ids().collect(),
        files,
        absent,
        signature: crate::signature_check::signature_format(&repo, oid),
    })
}

/// One file's hunks, with every change shown. Git only: `highlight` and `intraline` are left
/// empty for the caller to fill in with `highlight::highlight_diff` and `intraline::intraline`.
pub fn load_diff(workdir: &Path, target: &DiffTarget) -> Result<FileDiff> {
    load_diff_with(workdir, target, LoadDiffOptions::default())
}

/// Context for the whole-file load: more lines than any real file has, so libgit2 returns one
/// hunk covering the whole file. Not u32::MAX: xdiff keeps the context in a C long (32 bits on
/// Windows) and doubles it.
const WHOLE_FILE_CONTEXT: u32 = 1 << 24;

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
    load(workdir, target, opts, false).map(|v| v.diff)
}

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
}

/// `load_diff_with` plus the old file's text, for highlighting the hunks in context
/// (`highlight::highlight_in_context`), and the path's `linguist-language` gitattribute (for
/// commit and comparison diffs from the commit's own `.gitattributes` files, else from the
/// working tree, then the index; `None` for binary files). The text is `None` for added,
/// untracked or binary files, a whole-file load (its one hunk is the whole file), an old blob
/// over `highlight::OLD_TEXT_MAX_BYTES`, or one that doesn't match the hunks.
pub fn load_diff_for_view(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions) -> Result<ViewDiff> {
    load(workdir, target, opts, true)
}

fn load(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions, with_old: bool) -> Result<ViewDiff> {
    let repo = Repository::open(workdir)?;
    let path = target.path();
    let diff = target_diff(&repo, target, opts, None)?;
    let lfs = with_old.then(|| super::lfs_marks::diff_change(&repo, target, &diff)).flatten();
    let mut out = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    let Some(patch) = file_patch(&diff, path)? else {
        out.binary = true;
        return Ok(ViewDiff { diff: out, old: None, language: None, lfs });
    };
    out.binary = patch.delta().flags().is_binary();
    for h in 0..patch.num_hunks() {
        out.hunks.push(hunk_of(&patch, h)?);
    }
    if opts.whole_file && !out.binary && !out.hunks.is_empty() {
        let diff = target_diff(&repo, target, opts, Some(WHOLE_FILE_CONTEXT))?;
        if let Some(patch) = file_patch(&diff, path)?
            && patch.num_hunks() == 1
        {
            let whole = hunk_of(&patch, 0)?;
            out.whole = crate::whole_file::WholeFile::new(&out.hunks, whole).map(Box::new);
        }
    }
    // `patch` is still the first load's (the whole-file block's is scoped to it).
    let old = if with_old && !out.binary && out.whole.is_none() { old_text(&repo, &patch, &out.hunks) } else { None };
    let language = if with_old && !out.binary { super::attrs::linguist_language_for(&repo, target, path) } else { None };
    Ok(ViewDiff { diff: out, old, language, lfs })
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

/// The diff behind `target`, with `context` lines around each change when set (and hunks
/// never merged because of nearby changes, so one hunk spans the file).
pub(super) fn target_diff<'r>(repo: &'r Repository, target: &DiffTarget, opts: LoadDiffOptions, context: Option<u32>) -> Result<Diff<'r>> {
    let mut o = match target {
        DiffTarget::Commit(..) | DiffTarget::Between(..) | DiffTarget::ToWorkdir(..) => DiffOptions::new(),
        DiffTarget::Unstaged(p) => workdir_opts(p),
        DiffTarget::Staged(p) => path_opts(p),
    };
    whitespace_opts(&mut o, opts);
    if let Some(n) = context {
        o.context_lines(n).interhunk_lines(0);
    }
    Ok(match target {
        DiffTarget::Commit(oid, _) => commit_diff(repo, *oid, Some(&mut o))?,
        DiffTarget::Between(from, to, _) => trees_diff(repo, *from, *to, Some(&mut o))?,
        DiffTarget::ToWorkdir(oid, _) => workdir_diff(repo, *oid, Some(&mut o))?,
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
    let idx =
        diff.deltas().position(|d| super::rpath::touches(&d, path)).ok_or_else(|| Error::NotFound(format!("no changes in {path}")))?;
    Ok(Patch::from_diff(diff, idx)?)
}

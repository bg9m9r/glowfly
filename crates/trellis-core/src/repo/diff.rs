//! Commit details, comparisons between two commits, file diffs and per-commit line counts.

use super::*;
use crate::error::{Error, Result};
use git2::{Delta, Diff, DiffOptions, Oid, Patch, Repository, Tree};
use std::path::Path;

/// Lines added and removed, and files changed, for each commit (against its first parent).
pub fn commit_stats(workdir: &Path, oids: &[Oid]) -> Vec<(Oid, usize, usize, usize)> {
    let Ok(repo) = Repository::open(workdir) else { return Vec::new() };
    oids.iter()
        .filter_map(|&oid| {
            let stats = commit_diff(&repo, oid, None).ok()?.stats().ok()?;
            Some((oid, stats.files_changed(), stats.insertions(), stats.deletions()))
        })
        .collect()
}

fn delta_char(d: Delta) -> char {
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

/// One entry per delta, named by its new path (the old one for a deletion).
fn file_entries(diff: &Diff) -> Vec<FileEntry> {
    diff.deltas()
        .map(|d| FileEntry {
            path: d.new_file().path().or(d.old_file().path()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            status: delta_char(d.status()),
        })
        .collect()
}

/// The files that differ from commit `from` to commit `to`: a file only in `to` is 'A', only in
/// `from` is 'D', and a rename is 'R' under its new path. In libgit2 path order.
pub fn compare_files(workdir: &Path, from: Oid, to: Oid) -> Result<Vec<FileEntry>> {
    let repo = Repository::open(workdir)?;
    Ok(file_entries(&trees_diff(&repo, from, to, None)?))
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
    let files = file_entries(&commit_diff(&repo, oid, None)?);
    Ok(CommitDetail {
        oid,
        message: c.message().unwrap_or_default().trim_end().to_owned(),
        author: c.author().name().unwrap_or_default().to_owned(),
        email: c.author().email().unwrap_or_default().to_owned(),
        committer: c.committer().name().unwrap_or_default().to_owned(),
        time: c.time().seconds(),
        parents: c.parent_ids().collect(),
        files,
    })
}

/// One file's hunks, with every change shown. Git only: `highlight` and `intraline` are left
/// empty for the caller to fill in with `highlight::highlight_diff` and `intraline::intraline`.
pub fn load_diff(workdir: &Path, target: &DiffTarget) -> Result<FileDiff> {
    load_diff_with(workdir, target, LoadDiffOptions::default())
}

/// `load_diff` with options. With whitespace ignored, a modified file whose changes are all
/// whitespace comes back with `hunks` empty rather than an error: the delta stays (deltas compare
/// blob ids) and only the patch loses its hunks. Hunk indexes from such a diff don't match
/// `apply_hunk` or `apply_lines`, which always work on the unfiltered diff.
pub fn load_diff_with(workdir: &Path, target: &DiffTarget, opts: LoadDiffOptions) -> Result<FileDiff> {
    let repo = Repository::open(workdir)?;
    let path = target.path();
    let diff = match target {
        DiffTarget::Commit(oid, _) => commit_diff(&repo, *oid, Some(whitespace_opts(&mut DiffOptions::new(), opts)))?,
        DiffTarget::Between(from, to, _) => trees_diff(&repo, *from, *to, Some(whitespace_opts(&mut DiffOptions::new(), opts)))?,
        DiffTarget::Unstaged(p) => repo.diff_index_to_workdir(None, Some(whitespace_opts(&mut workdir_opts(p), opts)))?,
        DiffTarget::Staged(p) => {
            repo.diff_tree_to_index(head_tree(&repo).as_ref(), None, Some(whitespace_opts(&mut path_opts(p), opts)))?
        }
    };
    let mut out = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default() };
    let Some(patch) = file_patch(&diff, path)? else {
        out.binary = true;
        return Ok(out);
    };
    out.binary = patch.delta().flags().is_binary();
    for h in 0..patch.num_hunks() {
        out.hunks.push(hunk_of(&patch, h)?);
    }
    Ok(out)
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
            text: String::from_utf8_lossy(line.content()).trim_end_matches(['\n', '\r']).replace('\t', "    "),
        });
    }
    Ok(Hunk { header: String::from_utf8_lossy(hunk.header()).trim_end().to_owned(), lines })
}

/// The patch for `path` in `diff`: `NotFound` when the file has no changes, `None` when it is
/// binary. Shared by `load_diff` and line staging, so both see the same hunks and lines.
pub(super) fn file_patch<'b>(diff: &Diff<'b>, path: &str) -> Result<Option<Patch<'b>>> {
    let matches = |p: Option<&Path>| p.is_some_and(|p| p.to_string_lossy() == path);
    let idx = diff
        .deltas()
        .position(|d| matches(d.new_file().path()) || matches(d.old_file().path()))
        .ok_or_else(|| Error::NotFound(format!("no changes in {path}")))?;
    Ok(Patch::from_diff(diff, idx)?)
}

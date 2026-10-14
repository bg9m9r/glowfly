//! Commit details, file diffs and per-commit line counts.

use super::*;
use crate::error::{Error, Result};
use git2::{Delta, Diff, DiffOptions, Oid, Patch, Repository, Tree};
use std::path::Path;

/// Lines added and removed, and files changed, for each commit (against its first parent).
pub fn commit_stats(workdir: &Path, oids: &[Oid]) -> Vec<(Oid, usize, usize, usize)> {
    let Ok(repo) = Repository::open(workdir) else { return Vec::new() };
    oids.iter()
        .filter_map(|&oid| {
            let stats = commit_diff(&repo, oid).ok()?.stats().ok()?;
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

fn commit_diff<'r>(repo: &'r Repository, oid: Oid) -> Result<Diff<'r>> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let parent_tree = if c.parent_count() > 0 { Some(c.parent(0)?.tree()?) } else { None };
    let mut diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    diff.find_similar(None)?;
    Ok(diff)
}

pub(super) fn path_opts(path: &str) -> DiffOptions {
    let mut o = DiffOptions::new();
    o.pathspec(path).disable_pathspec_match(true);
    o
}

pub(super) fn workdir_opts(path: &str) -> DiffOptions {
    let mut o = path_opts(path);
    o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
    o
}

pub fn commit_detail(workdir: &Path, oid: Oid) -> Result<CommitDetail> {
    let repo = Repository::open(workdir)?;
    let c = repo.find_commit(oid)?;
    let files = commit_diff(&repo, oid)?
        .deltas()
        .map(|d| FileEntry {
            path: d.new_file().path().or(d.old_file().path()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            status: delta_char(d.status()),
        })
        .collect();
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

/// One file's hunks. Git only: `highlight` is left `None` for the caller to fill in with
/// `highlight::highlight_diff`.
pub fn load_diff(workdir: &Path, target: &DiffTarget) -> Result<FileDiff> {
    let repo = Repository::open(workdir)?;
    let path = target.path();
    let diff = match target {
        DiffTarget::Commit(oid, _) => commit_diff(&repo, *oid)?,
        DiffTarget::Unstaged(p) => repo.diff_index_to_workdir(None, Some(&mut workdir_opts(p)))?,
        DiffTarget::Staged(p) => repo.diff_tree_to_index(head_tree(&repo).as_ref(), None, Some(&mut path_opts(p)))?,
    };
    let mut out = FileDiff { binary: false, hunks: Vec::new(), highlight: None };
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

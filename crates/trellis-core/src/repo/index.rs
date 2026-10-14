//! Index and working-tree edits: staging, unstaging and discarding files and hunks.

use super::*;
use crate::error::{Error, Result};
use crate::gitcli::{NO_PROMPTS, args, run_with_input};
use git2::{ApplyLocation, ApplyOptions, Repository};
use std::path::Path;

/// Runs `git --literal-pathspecs <a>` in `workdir`, so paths are never read as globs. Staging
/// and discarding whole files goes through git rather than libgit2 because only git runs the
/// external clean/smudge filters (Git LFS, git-crypt) and `working-tree-encoding` conversions:
/// without them, staging an LFS file would store its binary content instead of the pointer.
pub(super) fn git(workdir: &Path, a: &[&str]) -> Result<()> {
    let mut argv = args(&["--literal-pathspecs"]);
    argv.extend(args(a));
    let (ok, out) = run_with_input(workdir, &argv, "", NO_PROMPTS);
    if ok { Ok(()) } else { Err(Error::Other(if out.is_empty() { format!("git {} failed", a.join(" ")) } else { out })) }
}

/// Stages, unstages or discards a single hunk (by index within the displayed diff). The index
/// counts hunks of the unfiltered diff, so callers must not pass an index taken from a
/// `load_diff_with` diff that ignores whitespace. `expected` is that hunk as the user saw it:
/// unless git produces exactly the same hunk now (header and every line), the index may name
/// another hunk, so the operation is refused as stale with `NotFound` and nothing changes.
pub fn apply_hunk(workdir: &Path, path: &str, hunk_index: usize, expected: &Hunk, op: HunkOp) -> Result<()> {
    let repo = Repository::open(workdir)?;
    if op != HunkOp::Unstage {
        // Unstaging only moves blobs between HEAD and the index, where no conversion applies.
        attrs::refuse_converted(&repo, path, if op == HunkOp::Stage { "staging" } else { "discarding" })?;
    }
    // The forward diff `load_diff` shows, to check the hunk; the one applied may be reversed.
    let shown = match op {
        HunkOp::Stage | HunkOp::Discard => repo.diff_index_to_workdir(None, Some(&mut workdir_opts(path)))?,
        HunkOp::Unstage => repo.diff_tree_to_index(head_tree(&repo).as_ref(), None, Some(&mut path_opts(path)))?,
    };
    let stale = || Error::NotFound("the diff changed; try again on the updated diff".into());
    let patch = diff::file_patch(&shown, path)?.ok_or_else(stale)?;
    if hunk_index >= patch.num_hunks() || diff::hunk_of(&patch, hunk_index)? != *expected {
        return Err(stale());
    }
    let (diff, location) = match op {
        HunkOp::Stage => (shown, ApplyLocation::Index),
        HunkOp::Discard => {
            let mut o = workdir_opts(path);
            o.reverse(true);
            (repo.diff_index_to_workdir(None, Some(&mut o))?, ApplyLocation::WorkDir)
        }
        HunkOp::Unstage => {
            let mut o = path_opts(path);
            o.reverse(true);
            let tree = head_tree(&repo);
            (repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut o))?, ApplyLocation::Index)
        }
    };
    let mut seen = 0usize;
    let mut opts = ApplyOptions::new();
    opts.hunk_callback(|_| {
        let keep = seen == hunk_index;
        seen += 1;
        keep
    });
    repo.apply(&diff, location, Some(&mut opts))?;
    Ok(())
}

/// Stages `path` as `git add` does: its new content, or its removal when it is gone.
pub fn stage_file(workdir: &Path, path: &str) -> Result<()> {
    git(workdir, &["add", "--all", "--", path])
}

/// Stages every change in the working tree, new files included (ignored ones are skipped).
pub fn stage_all(workdir: &Path) -> Result<()> {
    git(workdir, &["add", "--all"])
}

pub fn unstage_paths(workdir: &Path, paths: &[String]) -> Result<()> {
    let repo = Repository::open(workdir)?;
    match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
        Some(head) => repo.reset_default(Some(head.as_object()), paths.iter().map(String::as_str))?,
        None => {
            // No commits yet: unstaging means removing the entry from the index.
            let mut index = repo.index()?;
            for p in paths {
                index.remove_path(Path::new(p))?;
            }
            index.write()?;
        }
    }
    Ok(())
}

/// Reverts a tracked file's unstaged changes to the index version.
pub fn discard_file(workdir: &Path, path: &str) -> Result<()> {
    git(workdir, &["restore", "--worktree", "--", path])
}

/// Discards the untracked file `path`: deletes it the way `removal` asks (to the trash, or
/// for good) and prunes the folders it leaves empty. Refused, changing nothing, when `path`
/// has an index entry (it is tracked or staged now), when the trash can't take it
/// (`Error::NoTrash`), and for every reason `delete_file` refuses.
pub fn discard_untracked(workdir: &Path, path: &str, removal: Removal) -> Result<Deleted> {
    let rel = workfile::contained(path)?;
    let repo = Repository::open(workdir)?;
    let idx = repo.index()?;
    if (0..=3).any(|s| idx.get_path(Path::new(rel), s).is_some()) {
        return Err(Error::Other(format!("{rel} is tracked now; refresh and try again")));
    }
    // Windows keeps files locked while they are held open.
    drop(idx);
    drop(repo);
    workfile::delete_file(workdir, rel, removal)
}

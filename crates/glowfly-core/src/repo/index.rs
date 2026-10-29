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
/// `load_diff_with` diff that ignores whitespace (`apply_shown` takes those). `expected` is that hunk as the user saw it:
/// unless git produces exactly the same hunk now (header and every line), the index may name
/// another hunk, so the operation is refused as stale with `NotFound` and nothing changes.
pub fn apply_hunk(workdir: &Path, path: &str, hunk_index: usize, expected: &Hunk, op: HunkOp) -> Result<()> {
    if op == HunkOp::Discard {
        collide::refuse(workdir, path, "discard changes to")?;
    }
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
    // Only `path`'s own delta: with `core.ignorecase` the diff can also hold a path that differs
    // only in case, and hunks are counted across every delta that is applied.
    let mut seen = 0usize;
    let mut opts = ApplyOptions::new();
    opts.delta_callback(|d| d.is_some_and(|d| rpath::touches(&d, path)));
    opts.hunk_callback(|_| {
        let keep = seen == hunk_index;
        seen += 1;
        keep
    });
    match location {
        ApplyLocation::WorkDir => repo.apply(&diff, location, Some(&mut opts))?,
        _ => exact::apply_to_index(&repo, &diff, path, &mut opts)?,
    }
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

/// Unstages each of `paths` (exact paths, never globs or case-folded matches): its index entry
/// goes back to HEAD's version, or out of the index when HEAD doesn't have it (or there is no
/// commit yet). One index write for all of them.
pub fn unstage_paths(workdir: &Path, paths: &[String]) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let head = head_tree(&repo);
    let mut index = exact::index(&repo)?;
    for p in paths {
        exact::reset_path(&repo, &mut index, head.as_ref(), p)?;
    }
    index.write()?;
    Ok(())
}

/// Reverts a tracked file's unstaged changes to the index version.
pub fn discard_file(workdir: &Path, path: &str) -> Result<()> {
    collide::refuse(workdir, path, "discard changes to")?;
    git(workdir, &["restore", "--worktree", "--", path])
}

/// Discards the untracked file `path`: deletes it the way `removal` asks (to the trash, or
/// for good) and prunes the folders it leaves empty. Refused, changing nothing, when `path`
/// has an index entry (it is tracked or staged now), when the trash can't take it
/// (`Error::TrashRefused`), and for every reason `delete_file` refuses.
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

/// What discarding every line of `path`'s unstaged diff does, once checked against the diff the
/// user saw (`expected`, its only hunk, loaded with whitespace changes hidden or not).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WholeDiscard {
    /// An untracked file: it is deleted ([`discard_untracked`]), not left empty.
    Delete,
    /// Marked intent-to-add (`git add -N`): git has it, so its lines are discarded like any
    /// others and it stays, empty and added.
    Lines,
}

/// Checks that discarding every line of `path` still means discarding the whole file the user
/// saw: git produces exactly `expected` as the unstaged diff's only hunk, every changed line of
/// it added. Refused as stale (`NotFound`) otherwise, so a file that changed since is never
/// deleted; and refused when the path is tracked or staged by now.
pub fn whole_discard(workdir: &Path, path: &str, expected: &Hunk, ignore_whitespace: bool) -> Result<WholeDiscard> {
    let rel = workfile::contained(path)?;
    let stale = || Error::NotFound("the diff changed; select the lines again".into());
    let added = |h: &Hunk| h.lines.iter().all(|l| matches!(l.origin, '+' | '=' | '<' | '>')) && h.lines.iter().any(|l| l.origin == '+');
    let repo = Repository::open(workdir)?;
    let opts = LoadDiffOptions { ignore_whitespace, ..Default::default() };
    let diff = super::diff::target_diff(&repo, &DiffTarget::Unstaged(path.into()), opts, None)?;
    let patch = super::diff::file_patch(&diff, path).map_err(|_| stale())?.ok_or_else(stale)?;
    if patch.num_hunks() != 1 || super::diff::hunk_of(&patch, 0)? != *expected || !added(expected) {
        return Err(stale());
    }
    let index = repo.index()?;
    let intent = git2::IndexEntryExtendedFlag::INTENT_TO_ADD.bits();
    match (0..=3).find_map(|s| index.get_path(Path::new(rel), s)) {
        None => Ok(WholeDiscard::Delete),
        Some(e) if e.flags_extended & intent != 0 => Ok(WholeDiscard::Lines),
        Some(_) => Err(Error::Other(format!("{rel} is tracked now; refresh and try again"))),
    }
}

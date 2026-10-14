//! Index and working-tree edits: staging, unstaging and discarding files and hunks.

use super::*;
use crate::error::Result;
use git2::{ApplyLocation, ApplyOptions, IndexAddOption, Repository, build::CheckoutBuilder};
use std::path::Path;

/// Stages, unstages or discards a single hunk (by index within the displayed diff).
pub fn apply_hunk(workdir: &Path, path: &str, hunk_index: usize, op: HunkOp) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let (diff, location) = match op {
        HunkOp::Stage => (repo.diff_index_to_workdir(None, Some(&mut workdir_opts(path)))?, ApplyLocation::Index),
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

pub fn stage_file(workdir: &Path, path: &str) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = repo.index()?;
    if workdir.join(path).exists() {
        index.add_path(Path::new(path))?;
    } else {
        index.remove_path(Path::new(path))?;
    }
    index.write()?;
    Ok(())
}

pub fn stage_all(workdir: &Path) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    index.write()?;
    Ok(())
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

/// Reverts a file's unstaged changes to the index version, or deletes it if untracked.
pub fn discard_file(workdir: &Path, path: &str, untracked: bool) -> Result<()> {
    if untracked {
        std::fs::remove_file(workdir.join(path))?;
        return Ok(());
    }
    let repo = Repository::open(workdir)?;
    let mut cb = CheckoutBuilder::new();
    cb.force().path(path);
    repo.checkout_index(None, Some(&mut cb))?;
    Ok(())
}

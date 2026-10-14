//! Restoring one file from a commit into the working tree, without touching the index or HEAD.

use crate::error::{Error, Result};
use git2::build::CheckoutBuilder;
use git2::{ErrorCode, IndexEntryExtendedFlag, IndexEntryFlag, ObjectType, Oid, Repository, Status, Tree};
use std::path::Path;

/// Where a restored file's contents came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoredFrom {
    /// The commit that was asked for.
    Commit,
    /// The commit deleted the file, so its first parent's version (this commit) was written.
    Parent(Oid),
}

/// The git mode of a submodule entry (a gitlink).
const GITLINK: i32 = 0o160000;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Writes `path` as it is in commit `oid` into the working tree, leaving the index and HEAD alone.
/// When the commit doesn't have `path` (it deleted it), the version in its first parent is written.
///
/// Nothing is written when the path is a submodule or a folder in that commit, when the index
/// has unresolved conflicts for it, or when the working tree has a folder at the path or a file
/// where one of its parent folders should be. The write goes through libgit2's checkout, so line
/// endings (`core.autocrlf`), the executable bit and symbolic links come out as git would write
/// them.
///
/// Unless `overwrite` is set, nothing is written when the working-tree file holds contents that
/// neither the index nor a commit can bring back (it is untracked or ignored, has unstaged edits,
/// or has edits git is told to skip); that refusal is [`Error::WouldOverwrite`].
pub fn restore_file(workdir: &Path, oid: Oid, path: &str, overwrite: bool) -> Result<RestoredFrom> {
    let repo = Repository::open(workdir)?;
    let root = repo.workdir().ok_or_else(|| Error::Unsupported("a bare repository has no working tree".into()))?.to_path_buf();
    let commit = repo.find_commit(oid)?;
    let (tree, from) = source_tree(&commit, path)?;

    let entry = tree.get_path(Path::new(path))?;
    if entry.filemode() == GITLINK {
        return Err(Error::Unsupported(format!("{path} is a submodule; it can't be restored as a file")));
    }
    if entry.kind() == Some(ObjectType::Tree) {
        return Err(Error::Unsupported(format!("{path} is a folder in that commit")));
    }

    let index = repo.index()?;
    if (1..=3).any(|stage| index.get_path(Path::new(path), stage).is_some()) {
        return Err(Error::Other(format!("{path} has unresolved conflicts; resolve them first")));
    }

    // A forced checkout would delete whatever is in the way; refuse instead.
    if root.join(path).symlink_metadata().is_ok_and(|m| m.is_dir()) {
        return Err(Error::Other(format!("a folder named {path} is in the way")));
    }
    let mut prefix = root.clone();
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts[..parts.len() - 1].iter().enumerate() {
        prefix.push(part);
        if prefix.symlink_metadata().is_ok_and(|m| !m.is_dir()) {
            let component = parts[..=i].join("/");
            return Err(Error::Other(format!("{component} is a file, so {path} can't be written")));
        }
    }

    if !overwrite && let Some(reason) = unrecoverable(&repo, &root, path)? {
        return Err(Error::WouldOverwrite(reason));
    }

    let mut cb = CheckoutBuilder::new();
    // update_index(false): the restored file must show up as an unstaged change, not be staged.
    cb.force().update_index(false).disable_pathspec_match(true).path(path);
    repo.checkout_tree(tree.as_object(), Some(&mut cb))?;
    Ok(from)
}

/// Why overwriting the working-tree file at `path` would lose contents nothing else holds, or
/// `None` when nothing is at the path or the file matches the index (so discarding brings it back).
/// Looks at the disk itself: ignored files and skip-worktree or assume-unchanged entries are
/// left out of status, so status alone would call them clean.
fn unrecoverable(repo: &Repository, root: &Path, path: &str) -> Result<Option<String>> {
    let full = root.join(path);
    if full.symlink_metadata().is_err() {
        return Ok(None);
    }
    let index = repo.index()?;
    let Some(entry) = index.get_path(Path::new(path), 0) else {
        let why = if repo.status_should_ignore(Path::new(path))? { "is ignored by git" } else { "isn't tracked by git" };
        return Ok(Some(format!("{path} {why}, so nothing else holds its current contents")));
    };
    let skipped = IndexEntryFlag::from_bits_truncate(entry.flags).contains(IndexEntryFlag::VALID)
        || IndexEntryExtendedFlag::from_bits_truncate(entry.flags_extended).contains(IndexEntryExtendedFlag::SKIP_WORKTREE);
    if skipped {
        // Status won't report edits git is told to skip, so compare the bytes instead. A file
        // that only differs by line-ending conversion counts as changed, which errs on the safe side.
        let same =
            full.symlink_metadata().is_ok_and(|m| m.is_file()) && Oid::hash_file(ObjectType::Blob, &full).is_ok_and(|id| id == entry.id);
        return Ok((!same).then(|| format!("{path} is marked so that git skips its local edits, so status doesn't show them")));
    }
    let changed = Status::WT_MODIFIED | Status::WT_TYPECHANGE | Status::WT_RENAMED | Status::WT_NEW;
    Ok(repo.status_file(Path::new(path))?.intersects(changed).then(|| format!("{path} has uncommitted changes")))
}

/// The tree to restore `path` from: the commit's, or its first parent's when the commit
/// doesn't have the path.
fn source_tree<'r>(commit: &git2::Commit<'r>, path: &str) -> Result<(Tree<'r>, RestoredFrom)> {
    let tree = commit.tree()?;
    match tree.get_path(Path::new(path)) {
        Ok(_) => return Ok((tree, RestoredFrom::Commit)),
        Err(e) if e.code() != ErrorCode::NotFound => return Err(e.into()),
        Err(_) => {}
    }
    if commit.parent_count() > 0 {
        let parent = commit.parent(0)?;
        let parent_tree = parent.tree()?;
        if parent_tree.get_path(Path::new(path)).is_ok() {
            return Ok((parent_tree, RestoredFrom::Parent(parent.id())));
        }
    }
    Err(Error::NotFound(format!("{path} is not in commit {} or its parent", short(commit.id()))))
}

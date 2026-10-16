//! Creating a worktree: the `git worktree add` command lines, the default folder's name, and
//! checks of the folder it goes in. Everything here is pure except `check_folder`; the app runs
//! the command through `gitcli` so hooks, config and credential prompts apply. Removing,
//! locking, unlocking and pruning worktrees are in `manage`.

use std::path::{Path, PathBuf};

mod manage;
pub use manage::*;

/// What a new worktree checks out.
#[derive(Clone, Debug, PartialEq)]
pub enum WorktreeBase {
    /// A local branch (short name).
    Branch(String),
    /// A commit.
    Commit(git2::Oid),
}

impl WorktreeBase {
    /// The branch name, or the commit's short id.
    pub fn label(&self) -> String {
        match self {
            WorktreeBase::Branch(b) => b.clone(),
            WorktreeBase::Commit(oid) => short(*oid),
        }
    }
}

fn short(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// `git worktree add` for `folder`: a branch base without a new branch checks that branch out
/// (by its short name, so git doesn't detach), a commit base without one gives a detached HEAD,
/// and `new_branch` creates a branch at the base.
pub fn add_cmd(folder: &Path, base: &WorktreeBase, new_branch: Option<&str>) -> Vec<String> {
    let folder = folder.display().to_string();
    let mut cmd = vec!["worktree".to_owned(), "add".to_owned()];
    match (base, new_branch) {
        (_, Some(new)) => cmd.extend(["-b".to_owned(), new.to_owned()]),
        (WorktreeBase::Commit(_), None) => cmd.push("--detach".to_owned()),
        (WorktreeBase::Branch(_), None) => {}
    }
    cmd.push(folder);
    cmd.push(match base {
        WorktreeBase::Branch(b) => b.clone(),
        WorktreeBase::Commit(oid) => oid.to_string(),
    });
    cmd
}

/// A folder name made from a branch name: characters that can't be in a file name, and
/// whitespace, become '-', and leading or trailing '-' and '.' are dropped.
pub fn folder_slug(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_whitespace() { '-' } else { c })
        .collect();
    mapped.trim_matches(|c| c == '-' || c == '.').to_owned()
}

/// Why a folder can't hold a new worktree.
#[derive(Debug, PartialEq)]
pub enum FolderProblem {
    Empty,
    NotAbsolute,
    /// A file is at the path.
    IsFile,
    /// A folder with something in it is at the path.
    NotEmpty,
    /// The path couldn't be read; the OS error.
    Unreadable(String),
}

impl FolderProblem {
    /// The message shown for the folder `path`.
    pub fn message(&self, path: &str) -> String {
        match self {
            FolderProblem::Empty => "Choose a folder for the new worktree.".to_owned(),
            FolderProblem::NotAbsolute => "Enter the full path of the folder.".to_owned(),
            FolderProblem::IsFile => format!("{path} is a file. Choose a folder that doesn't exist yet, or an empty one."),
            FolderProblem::NotEmpty => format!("{path} already exists and isn't empty. Choose a new or empty folder."),
            FolderProblem::Unreadable(e) => format!("Can't read {path}: {e}"),
        }
    }
}

/// The typed folder, trimmed, when it is non-empty and absolute.
pub fn parse_folder(text: &str) -> Result<PathBuf, FolderProblem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FolderProblem::Empty);
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(FolderProblem::NotAbsolute);
    }
    Ok(path)
}

/// Whether git can create a worktree at `path`: nothing there, or an empty folder.
pub fn check_folder(path: &Path) -> Result<(), FolderProblem> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(FolderProblem::Unreadable(e.to_string())),
    };
    if !meta.is_dir() {
        return Err(FolderProblem::IsFile);
    }
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                Ok(())
            } else {
                Err(FolderProblem::NotEmpty)
            }
        }
        Err(e) => Err(FolderProblem::Unreadable(e.to_string())),
    }
}

#[cfg(test)]
mod tests;

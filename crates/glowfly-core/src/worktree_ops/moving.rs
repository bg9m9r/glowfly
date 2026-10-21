//! Moving a worktree to another folder: the `git worktree move` command line, the check that
//! nothing is at the target yet, and `move_worktree`, which refuses a lock and a taken target
//! before git runs, creates the target's missing parent folders, and then judges the result by
//! listing the worktrees again.

use crate::gitcli;
use std::path::Path;

/// `git worktree move <from> <to>`.
pub fn move_cmd(from: &Path, to: &Path) -> Vec<String> {
    vec!["worktree".to_owned(), "move".to_owned(), from.display().to_string(), to.display().to_string()]
}

/// Why a worktree can't be moved to a folder.
#[derive(Clone, Debug, PartialEq)]
pub enum MoveProblem {
    /// Something (a file, a folder, a link) is already at the path. git would move the worktree
    /// inside an existing folder, which Trellis never asks for.
    Taken,
    /// The path couldn't be read; the OS error.
    Unreadable(String),
}

impl MoveProblem {
    /// The message shown for the target `path`.
    pub fn message(&self, path: &str) -> String {
        match self {
            MoveProblem::Taken => format!("Something is already at {path}. Choose a folder that doesn't exist yet."),
            MoveProblem::Unreadable(e) => format!("Can't read {path}: {e}"),
        }
    }
}

/// Whether a worktree can be moved to `to`: nothing may be there, not even an empty folder or a
/// dangling link. Another spelling of an existing folder (another case on a volume that ignores
/// case) counts as taken too.
pub fn check_move_target(to: &Path) -> Result<(), MoveProblem> {
    match std::fs::symlink_metadata(to) {
        Ok(_) => Err(MoveProblem::Taken),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(MoveProblem::Unreadable(e.to_string())),
    }
}

/// How a move ended.
#[derive(Clone, Debug, PartialEq)]
pub enum MoveOutcome {
    /// The worktree is listed at the new folder and no longer at the old one.
    Moved,
    /// Nothing changed: something is already at the target (in the output).
    Taken,
    /// Nothing changed, for another reason (in the output).
    Failed,
}

/// The outcome of `move_worktree` and its output: "$ git …" lines and git's text, or the reason
/// nothing ran.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveReport {
    pub outcome: MoveOutcome,
    pub output: String,
}

fn failed(output: String) -> MoveReport {
    MoveReport { outcome: MoveOutcome::Failed, output }
}

/// Moves the worktree at `from` of the repository `cwd` belongs to into the new folder `to`.
pub fn move_worktree(cwd: &Path, from: &Path, to: &Path) -> MoveReport {
    let listed = || crate::repo::worktrees_at(cwd).unwrap_or_default();
    let Some(wt) = listed().into_iter().find(|w| crate::paths::same_folder(&w.path, from)) else {
        return failed(format!("{} is no longer a worktree of this repository.", from.display()));
    };
    if wt.locked.is_some() {
        return failed("It is locked. Unlock it first.".to_owned());
    }
    match check_move_target(to) {
        Ok(()) => {}
        Err(MoveProblem::Taken) => {
            return MoveReport { outcome: MoveOutcome::Taken, output: MoveProblem::Taken.message(&to.display().to_string()) };
        }
        Err(p) => return failed(p.message(&to.display().to_string())),
    }
    if let Some(parent) = to.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return failed(format!("Can't create the folder {}: {e}", parent.display()));
    }
    let cmd = move_cmd(&wt.path, to);
    let (_, text) = gitcli::run(cwd, &cmd);
    let mut output = format!("$ git {}", cmd.join(" "));
    if !text.trim().is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    // The listing decides, not the exit code.
    let after = listed();
    let at = |p: &Path| after.iter().any(|w| crate::paths::same_folder(&w.path, p));
    let outcome = if at(to) && !at(&wt.path) { MoveOutcome::Moved } else { MoveOutcome::Failed };
    MoveReport { outcome, output }
}

#[cfg(test)]
#[path = "moving_tests.rs"]
mod tests;

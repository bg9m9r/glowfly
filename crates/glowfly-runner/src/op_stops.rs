//! Where a merge, pull, cherry-pick, revert or rebase that Glowfly runs started
//! (`undo::op_start`), taken just before its command, after any auto-stash in front of it. When
//! the operation stops in progress, the job reports it (`UndoResult::Stopped`) so the undo history
//! can outlive an abort that puts the repository back exactly there.

use super::git_ops::run_git;
use crate::effect::UndoResult;
use glowfly_core::undo::{self, MoveOp, OpStart, UndoKind};
use std::path::{Path, PathBuf};

/// Whether the job's last command is the operation it is recorded as, and one that can stop
/// for conflicts.
pub(super) fn can_stop(cmds: &[Vec<String>], record: Option<&UndoKind>) -> bool {
    let Some(kind @ UndoKind::Move { op, .. }) = record else { return false };
    *op != MoveOp::Rewrite && cmds.last().and_then(|c| UndoKind::classify(c)).as_ref() == Some(kind)
}

/// `run_git`, noting where the operation starts just before the last command runs.
pub(super) fn run_noting_start(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(PathBuf, Vec<u8>)>,
    prompts: bool,
) -> (bool, String, Option<OpStart>) {
    let Some((last, first)) = cmds.split_last() else {
        let (ok, output) = run_git(cwd, cmds, write_file, prompts);
        return (ok, output, None);
    };
    let (ok, mut output) = run_git(cwd, first, write_file, prompts);
    if !ok {
        return (false, output, None);
    }
    let start = undo::op_start(cwd);
    let (ok, more) = run_git(cwd, std::slice::from_ref(last), None, prompts);
    if !output.is_empty() && !more.is_empty() {
        output.push('\n');
    }
    output.push_str(&more);
    (ok, output, start)
}

/// The result of a job whose operation started at `start` and that failed (`ok` false) with an
/// operation now in progress: it stopped there.
pub(super) fn stopped(cwd: &Path, ok: bool, start: Option<OpStart>) -> Option<UndoResult> {
    start.filter(|_| !ok && undo::in_progress(cwd)).map(UndoResult::Stopped)
}

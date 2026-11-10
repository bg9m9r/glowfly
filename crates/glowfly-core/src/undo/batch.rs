//! Several files' content actions recorded as one undo entry (`Action::Batch`): each part is a
//! whole-file `Discard` or a `DeleteFile` (an untracked file discarded, or a working copy
//! deleted). Undo and redo plan every
//! part as it would be planned alone and carry the plans out in order (`Plan::Each`). A part
//! that already holds what this would put there (an earlier run stopped partway) is skipped, and
//! a part that holds neither refuses the whole entry, naming its file.

use super::planning::{plan_with, stale};
use super::{Action, Options, Plan};
use crate::error::Result;
use std::path::Path;

/// What a batch of files had done to them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchOp {
    /// Their unstaged changes discarded (tracked ones restored from the index, untracked ones
    /// deleted).
    Discard,
    /// Their working copies deleted (Delete..., Unstage and delete...).
    Delete,
}

impl BatchOp {
    /// The entry's description for `n` files (English, as the other actions' are).
    pub fn describe(self, n: usize) -> String {
        match self {
            BatchOp::Discard => format!("discard in {n} files"),
            BatchOp::Delete => format!("delete of {n} files"),
        }
    }
}

/// The undo entry for `parts`, done together as `op`: None when nothing was done, the part
/// itself when there is only one, else one `Action::Batch`.
pub fn batch(op: BatchOp, mut parts: Vec<Action>) -> Option<Action> {
    match parts.len() {
        0 => None,
        1 => parts.pop(),
        _ => Some(Action::Batch { op, parts }),
    }
}

/// The undo (or `redo`) of a batch: every part's own plan, in order. A part whose plan is
/// refused but whose opposite plan isn't already holds what this would put there and is
/// skipped; any other refusal refuses the whole batch.
pub(super) fn plan(workdir: &Path, parts: &[Action], redo: bool) -> Result<Plan> {
    let mut steps = Vec::new();
    for part in parts {
        match plan_with(workdir, part, redo, Options::default())? {
            Plan::Refuse(r) => {
                if matches!(plan_with(workdir, part, !redo, Options::default())?, Plan::Refuse(_)) {
                    return Ok(Plan::Refuse(r));
                }
            }
            p => steps.push((part.clone(), p)),
        }
    }
    if steps.is_empty() {
        return Ok(stale("Every file already holds what this would put back"));
    }
    Ok(Plan::Each(steps))
}

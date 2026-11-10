//! How a recorded action is named in tooltips and job labels, and which history rules it follows.

use super::{Action, Discarded, HeadPos, MoveOp, ResetMode, short};
use crate::branch_ref::short_ref as ref_label;

/// The first line of `message`, cut to 50 characters (with "…" when longer).
fn cut50(message: &str) -> String {
    let summary = message.lines().next().unwrap_or_default();
    let cut: String = summary.chars().take(50).collect();
    if summary.chars().count() > 50 { format!("{cut}…") } else { cut }
}

/// "a", "a and b", or "a, b and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

fn moved(op: MoveOp, detail: Option<&str>) -> String {
    match (op, detail) {
        (MoveOp::Pull, _) | (_, None) => op.noun().to_owned(),
        (MoveOp::Merge, Some(d)) => format!("merge of {}", ref_label(d)),
        (MoveOp::Rebase, Some(d)) => format!("rebase onto {}", ref_label(d)),
        // The detail is the whole phrase ("interactive rebase of main", "reword of 1a2b3c4").
        (MoveOp::Rewrite, Some(d)) => d.to_owned(),
        (MoveOp::CherryPick | MoveOp::Revert | MoveOp::Am, Some(d)) => format!("{} of {d}", op.noun()),
    }
}

impl Action {
    /// What the action did, for tooltips and job labels ("commit 'Fix build'").
    pub fn describe(&self) -> String {
        match self {
            Action::Checkout { created: Some(c), .. } => format!("checkout of new branch {}", c.name),
            Action::Checkout { to, .. } => match to {
                HeadPos::Branch(b) => format!("checkout of {b}"),
                HeadPos::Detached(o) => format!("checkout of {}", short(*o)),
            },
            Action::Commit { message, amend, .. } => {
                format!("{} '{}'", if *amend { "amend" } else { "commit" }, cut50(message))
            }
            Action::Reset { mode, to, .. } => format!("{} reset to {}", mode.name(), short(*to)),
            Action::DeleteBranch { name, .. } => format!("delete of branch {name}"),
            Action::DiscardAll { .. } => "Discard all changes".to_owned(),
            Action::Discard { path, what, .. } => match what {
                Discarded::File => format!("discard in {path}"),
                Discarded::Hunk => format!("discard of a hunk in {path}"),
                Discarded::Lines(1) => format!("discard of 1 line in {path}"),
                Discarded::Lines(n) => format!("discard of {n} lines in {path}"),
                Discarded::RevertedHunk => format!("revert of a hunk in {path}"),
                Discarded::RevertedLines(1) => format!("revert of 1 line in {path}"),
                Discarded::RevertedLines(n) => format!("revert of {n} lines in {path}"),
                Discarded::RevertedFile => format!("revert of the changes to {path}"),
            },
            Action::Restore { path, .. } => format!("restore of {path}"),
            Action::CreateBranch { name, .. } => format!("creation of branch {name}"),
            Action::CreateTag { name, .. } => format!("creation of tag {name}"),
            Action::DeleteTag { name, remotes, .. } if remotes.is_empty() => format!("delete of tag {name}"),
            Action::DeleteTag { name, remotes, .. } => format!("delete of tag {name} (here only, not on {})", and_list(remotes)),
            Action::Moved { op, detail, .. } => moved(*op, detail.as_deref()),
            Action::StashPush { message, .. } => format!("stash '{}'", cut50(message)),
            Action::StashApply { pop: true, message, .. } => format!("unstash of '{}'", cut50(message)),
            Action::StashApply { pop: false, message, .. } => format!("apply of stash '{}'", cut50(message)),
            Action::StashDrop { message, .. } => format!("drop of stash '{}'", cut50(message)),
            Action::Resolve { path, staged: Some(_), .. } => format!("resolution of {path}"),
            Action::Resolve { path, staged: None, .. } => format!("save of {path} with markers"),
            Action::Conflicts { change, files, .. } => super::marked::describe(*change, files),
            Action::DeleteFile { path, .. } => format!("delete of {path}"),
            Action::UnstageGone { path, .. } => format!("unstage of {path}"),
            Action::SetBranch { name, to, .. } => format!("move of branch {name} to {}", short(*to)),
            Action::Restacked { detail, .. } => detail.clone(),
            Action::AbortKept { op, .. } => format!("abort of {}", op.noun()),
            Action::Batch { op, parts } => op.describe(parts.len()),
            Action::RenameBranch { old, new, remote: None } => format!("rename of branch {old} to {new}"),
            Action::RenameBranch { old, new, remote: Some(r) } => format!("rename of branch {old} to {new} (here only, not on {r})"),
            Action::RenameRemote { old, new, .. } => format!("rename of remote {old} to {new}"),
            Action::RemoveRemote { name, .. } => format!("removal of remote {name}"),
            Action::AddWorktree { path, created } => {
                let folder = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                format!("creation of worktree {folder} with branch {}", created.name)
            }
        }
    }

    /// Whether the action threw away work that only Glowfly keeps (a discard, a deleted file, a
    /// dropped stash, an aborted operation's work): it stays undoable past a job that can't be
    /// undone, since its undo checks the files (or stash list) it writes to before it acts.
    pub fn is_content(&self) -> bool {
        matches!(
            self,
            Action::Discard { .. } | Action::DeleteFile { .. } | Action::StashDrop { .. } | Action::AbortKept { .. } | Action::Batch { .. }
        )
    }

    /// Whether the action stays recorded when its job failed or was stopped after it: a tag
    /// delete whose later remote deletes failed is still undoable here, as is a branch rename
    /// whose pushes failed, or a remote rename whose URL changes did.
    pub fn kept_when_failed(&self) -> bool {
        matches!(self, Action::DeleteTag { .. } | Action::RenameBranch { .. } | Action::RenameRemote { .. })
    }

    /// Whether a refusal for local changes can offer to stash them and undo anyway: for undoing
    /// a hard reset or Discard all (whose redo stashes again by itself), a merge, pull,
    /// cherry-pick, revert or rebase, or a stash. Never for a redo.
    pub fn offers_stash(&self, redo: bool) -> bool {
        !redo
            && matches!(
                self,
                Action::Reset { mode: ResetMode::Hard, .. } | Action::DiscardAll { .. } | Action::Moved { .. } | Action::StashPush { .. }
            )
    }

    /// Whether "Stash changes and undo" puts the stashed changes back once the undo is done
    /// (with `stash pop --index`): for a merge, pull, cherry-pick, revert or rebase. Undoing a
    /// hard reset, Discard all or a stash brings back other changes at the same paths, so the
    /// newer ones stay in the stash.
    pub fn restores_stash(&self) -> bool {
        matches!(self, Action::Moved { .. })
    }
}

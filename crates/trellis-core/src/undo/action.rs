//! Recorded operations and how they are named in tooltips and job labels.

use super::{MoveOp, ResetMode, Saved, short};
use git2::Oid;

/// Where HEAD points.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadPos {
    Branch(String),
    Detached(Oid),
}

/// A branch an operation created: its name, the commit it was made at and its upstream
/// (a remote-tracking branch such as "origin/x").
#[derive(Clone, Debug, PartialEq)]
pub struct Created {
    pub name: String,
    pub oid: Oid,
    pub upstream: Option<String>,
}

/// The changed paths of a working tree, sorted, each with the hash of its working file
/// (`None`: absent; zero for a folder such as a submodule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes(pub Vec<(String, Option<Oid>)>);

/// Which part of a file's unstaged changes a discard threw away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discarded {
    File,
    Hunk,
    /// This many selected lines.
    Lines(usize),
}

/// A recorded operation that can be undone (and then redone). `Discard` is one file's unstaged
/// changes discarded, `what` saying which part (all, a hunk or some lines): `before` is its
/// content then and `after` what the discard left (`None`: absent). `Restore` is one file
/// written from commit `oid`, with `before` the working copy it replaced and `after` what the
/// restore wrote. A hard reset's `stash` holds the changes it stashed. A checkout that made a
/// new branch (`-b`, `--track`) remembers it in `created`. `Moved` is a merge, pull,
/// cherry-pick, revert or rebase (also an interactive one, or a history edit) that took HEAD
/// from `from` to `to`, including a merge, cherry-pick or revert that stopped for conflicts and
/// was finished later. `StashApply` is a pop or apply onto a clean tree that left `left`.
/// `StashDrop` is a dropped stash, put back with `stash store` under its list `message`.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Checkout {
        from: HeadPos,
        to: HeadPos,
        created: Option<Created>,
    },
    Commit {
        branch: Option<String>,
        from: Oid,
        to: Oid,
        message: String,
        amend: bool,
    },
    Reset {
        mode: ResetMode,
        branch: Option<String>,
        from: Oid,
        to: Oid,
        stash: Option<Oid>,
    },
    DeleteBranch {
        name: String,
        oid: Oid,
        upstream: Option<String>,
    },
    DiscardAll {
        stash: Oid,
        head: Oid,
    },
    Discard {
        path: String,
        what: Discarded,
        before: Option<Saved>,
        after: Option<Saved>,
    },
    Restore {
        path: String,
        oid: Oid,
        before: Option<Saved>,
        after: Option<Saved>,
    },
    CreateBranch {
        name: String,
        oid: Oid,
        upstream: Option<String>,
    },
    /// `oid` is the ref's own target: the tag object for an annotated tag.
    CreateTag {
        name: String,
        oid: Oid,
    },
    Moved {
        op: MoveOp,
        detail: Option<String>,
        branch: Option<String>,
        from: Oid,
        to: Oid,
    },
    StashPush {
        stash: Oid,
        head: Oid,
        message: String,
        replay: Option<Vec<String>>,
    },
    StashApply {
        pop: bool,
        stash: Oid,
        head: Oid,
        message: String,
        left: Changes,
    },
    StashDrop {
        stash: Oid,
        message: String,
    },
}

/// The first line of `message`, cut to 50 characters (with "…" when longer).
fn cut50(message: &str) -> String {
    let summary = message.lines().next().unwrap_or_default();
    let cut: String = summary.chars().take(50).collect();
    if summary.chars().count() > 50 { format!("{cut}…") } else { cut }
}

/// A ref name as the user knows it: without refs/heads/ or refs/remotes/.
fn ref_label(name: &str) -> &str {
    name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).unwrap_or(name)
}

fn moved(op: MoveOp, detail: Option<&str>) -> String {
    match (op, detail) {
        (MoveOp::Pull, _) | (_, None) => op.noun().to_owned(),
        (MoveOp::Merge, Some(d)) => format!("merge of {}", ref_label(d)),
        (MoveOp::Rebase, Some(d)) => format!("rebase onto {}", ref_label(d)),
        // The detail is the whole phrase ("interactive rebase of main", "reword of 1a2b3c4").
        (MoveOp::Rewrite, Some(d)) => d.to_owned(),
        (MoveOp::CherryPick | MoveOp::Revert, Some(d)) => format!("{} of {d}", op.noun()),
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
            },
            Action::Restore { path, .. } => format!("restore of {path}"),
            Action::CreateBranch { name, .. } => format!("creation of branch {name}"),
            Action::CreateTag { name, .. } => format!("creation of tag {name}"),
            Action::Moved { op, detail, .. } => moved(*op, detail.as_deref()),
            Action::StashPush { message, .. } => format!("stash '{}'", cut50(message)),
            Action::StashApply { pop, message, .. } => {
                format!("{} of stash '{}'", if *pop { "pop" } else { "apply" }, cut50(message))
            }
            Action::StashDrop { message, .. } => format!("drop of stash '{}'", cut50(message)),
        }
    }

    /// Whether a refusal for local changes can offer to stash them and undo anyway: only for
    /// undoing a hard reset or Discard all, whose redo stashes again by itself.
    pub fn offers_stash(&self, redo: bool) -> bool {
        !redo && matches!(self, Action::Reset { mode: ResetMode::Hard, .. } | Action::DiscardAll { .. })
    }
}

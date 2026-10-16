//! Recorded operations and how they are named in tooltips and job labels.

use super::{ResetMode, Saved, short};
use git2::Oid;

/// Where HEAD points.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadPos {
    Branch(String),
    Detached(Oid),
}

/// A recorded operation that can be undone (and then redone). `Discard` is one file's unstaged
/// changes (all, a hunk or some lines) discarded: `before` is its content then and `after` what
/// the discard left (`None`: absent). `Restore` is one file written from commit `oid`, with
/// `before` the working copy it replaced and `after` what the restore wrote. A hard reset's
/// `stash` holds the changes it stashed.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Checkout { from: HeadPos, to: HeadPos },
    Commit { branch: Option<String>, from: Oid, to: Oid, message: String, amend: bool },
    Reset { mode: ResetMode, branch: Option<String>, from: Oid, to: Oid, stash: Option<Oid> },
    DeleteBranch { name: String, oid: Oid, upstream: Option<String> },
    DiscardAll { stash: Oid, head: Oid },
    Discard { path: String, before: Option<Saved>, after: Option<Saved> },
    Restore { path: String, oid: Oid, before: Option<Saved>, after: Option<Saved> },
}

/// The first line of `message`, cut to 50 characters (with "…" when longer).
fn cut50(message: &str) -> String {
    let summary = message.lines().next().unwrap_or_default();
    let cut: String = summary.chars().take(50).collect();
    if summary.chars().count() > 50 { format!("{cut}…") } else { cut }
}

impl Action {
    /// What the action did, for tooltips and job labels ("commit 'Fix build'").
    pub fn describe(&self) -> String {
        match self {
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
            Action::Discard { path, .. } => format!("discard in {path}"),
            Action::Restore { path, .. } => format!("restore of {path}"),
        }
    }
}

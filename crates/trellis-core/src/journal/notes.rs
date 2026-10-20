//! Trellis's own records ("notes"): written before an operation that rewrites history, each
//! labelling the state that came before it. They live in `<git dir>/trellis/journal.jsonl`, one
//! JSON object per line, in the worktree's own git dir for a linked worktree (HEAD is per
//! worktree there). Appended through `persist::append`, trimmed to the newest `MAX_NOTES`
//! lines through `persist::atomic_write` once the file passes `TRIM_AT` bytes. Lines that don't
//! parse (a torn last line, a newer version, an unknown kind) are skipped.

use crate::error::Result;
use crate::persist;
use crate::undo::{HeadPos, MoveOp, UndoKind};
use git2::{BranchType, Oid, Repository};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The notes file's format version.
pub const VERSION: u32 = 1;
/// How many notes a trim keeps.
pub const MAX_NOTES: usize = 500;
/// The file size past which it is trimmed.
pub const TRIM_AT: u64 = 256 * 1024;

/// The operation a note was written before.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Op {
    Rebase {
        onto: Option<String>,
    },
    /// An interactive rebase or a graph edit.
    Rewrite,
    Merge {
        from: Option<String>,
    },
    Pull,
    CherryPick {
        commits: Option<String>,
    },
    Revert {
        commits: Option<String>,
    },
    Reset {
        mode: String,
        to: String,
    },
    Amend,
    DeleteBranch {
        name: String,
    },
    /// The journal's Restore: `branch` moved to `to`, or HEAD (`None`) checked out at it.
    Restore {
        branch: Option<String>,
        to: String,
    },
    Undo {
        what: String,
    },
    Redo {
        what: String,
    },
}

/// A ref as it was before the operation: a branch, or HEAD (`None`: detached or HEAD's own row).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotedRef {
    pub branch: Option<String>,
    /// The commit, as a full hex id.
    pub oid: String,
}

impl NotedRef {
    pub fn oid(&self) -> Option<Oid> {
        Oid::from_str(&self.oid).ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub v: u32,
    /// Unix seconds, from the runner's clock.
    pub at: i64,
    pub op: Op,
    pub refs: Vec<NotedRef>,
}

/// A full commit id cut to seven characters; anything else as it is.
fn cut(rev: &str) -> String {
    super::events::short_rev(rev)
}

impl Op {
    /// The note for a job recognised as `kind` with command line `cmd`, or `None` for the kinds
    /// that don't rewrite anything (checkouts, commits, creating refs, stashes, discards).
    pub fn of_kind(kind: &UndoKind, cmd: &[String]) -> Option<Op> {
        Some(match kind {
            UndoKind::Move { op, detail } => match op {
                MoveOp::Rebase => Op::Rebase { onto: detail.clone() },
                MoveOp::Rewrite => Op::Rewrite,
                MoveOp::Merge => Op::Merge { from: detail.clone() },
                MoveOp::Pull => Op::Pull,
                MoveOp::CherryPick => Op::CherryPick { commits: detail.clone() },
                MoveOp::Revert => Op::Revert { commits: detail.clone() },
            },
            UndoKind::Reset(mode) => {
                let to = cmd.get(2).map(|r| cut(r)).unwrap_or_default();
                Op::Reset { mode: mode.flag().trim_start_matches('-').to_owned(), to }
            }
            UndoKind::Commit { amend: true } => Op::Amend,
            UndoKind::DeleteBranch(name) => Op::DeleteBranch { name: name.clone() },
            _ => return None,
        })
    }

    /// The branch other than HEAD's that the operation moves, when it names one.
    pub fn branch(&self) -> Option<&str> {
        match self {
            Op::DeleteBranch { name } => Some(name),
            Op::Restore { branch: Some(b), .. } => Some(b),
            _ => None,
        }
    }
}

/// The refs an operation may move, captured before it runs.
#[derive(Debug)]
pub struct Captured {
    op: Op,
    at: i64,
    head: (Option<HeadPos>, Option<Oid>),
    /// `op.branch()` and its tip then.
    branch: Option<(String, Option<Oid>)>,
}

fn branch_tip(repo: &Repository, name: &str) -> Option<Oid> {
    repo.find_branch(name, BranchType::Local).ok().and_then(|b| b.get().target())
}

/// Captures HEAD (its position and commit) and, for `op.branch()`, that branch's tip.
pub fn capture(workdir: &Path, op: Op, at: i64) -> Result<Captured> {
    let repo = Repository::open(workdir)?;
    let head = crate::undo::head(&repo);
    let branch = op.branch().map(|b| (b.to_owned(), branch_tip(&repo, b)));
    Ok(Captured { op, at, head, branch })
}

impl Captured {
    /// Whether HEAD or the captured branch has moved since the capture.
    pub fn moved(&self, workdir: &Path) -> bool {
        let Ok(repo) = Repository::open(workdir) else { return false };
        match &self.branch {
            Some((name, tip)) => branch_tip(&repo, name) != *tip,
            None => crate::undo::head(&repo) != self.head,
        }
    }

    /// The note: the captured branch, else HEAD's branch at its tip, else detached HEAD. A
    /// restore of HEAD itself notes HEAD.
    pub fn into_note(self) -> Note {
        let noted = |branch: Option<String>, oid: Option<Oid>| oid.map(|o| NotedRef { branch, oid: o.to_string() });
        let head_ref = match (&self.op, self.head) {
            (Op::Restore { branch: None, .. }, (_, oid)) => noted(None, oid),
            (_, (Some(HeadPos::Branch(b)), oid)) => noted(Some(b), oid),
            (_, (_, oid)) => noted(None, oid),
        };
        let refs = match self.branch {
            Some((name, tip)) => noted(Some(name), tip),
            None => head_ref,
        };
        Note { v: VERSION, at: self.at, op: self.op, refs: refs.into_iter().collect() }
    }
}

/// The notes file of the repository at `workdir` (its worktree's own git dir).
pub fn path(workdir: &Path) -> Result<PathBuf> {
    Ok(Repository::open(workdir)?.path().join("trellis").join("journal.jsonl"))
}

/// Appends `note` as one line, trimming the file to the newest `MAX_NOTES` lines once it is
/// larger than `TRIM_AT`.
pub fn append(workdir: &Path, note: &Note) -> std::io::Result<()> {
    let file = path(workdir).map_err(std::io::Error::other)?;
    let mut line = serde_json::to_string(note).map_err(std::io::Error::other)?;
    line.push('\n');
    persist::append(&file, line.as_bytes())?;
    if std::fs::metadata(&file)?.len() > TRIM_AT {
        let text = std::fs::read(&file)?;
        let text = String::from_utf8_lossy(&text);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let keep = &lines[lines.len().saturating_sub(MAX_NOTES)..];
        let mut out = keep.join("\n");
        out.push('\n');
        persist::atomic_write(&file, out.as_bytes())?;
    }
    Ok(())
}

/// The notes of the repository at `workdir`, oldest first; a missing file has none.
pub fn read(workdir: &Path) -> Vec<Note> {
    let Ok(file) = path(workdir) else { return Vec::new() };
    let Ok(bytes) = std::fs::read(file) else { return Vec::new() };
    parse(&String::from_utf8_lossy(&bytes))
}

/// The notes in a file's `text`, skipping lines that don't parse or have another version.
pub fn parse(text: &str) -> Vec<Note> {
    text.lines().filter_map(|l| serde_json::from_str::<Note>(l).ok()).filter(|n| n.v == VERSION).collect()
}

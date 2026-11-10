//! Recorded operations: what each one remembers to be undone and redone. How they are named and
//! which history rules they follow is in `action_words.rs`.

use super::{ConflictChange, ConflictFile, DeleteVia, MoveOp, RemoteConfig, ResetMode, Saved, UrlChange};
use crate::repo::{ConflictIds, StagedNew};
use git2::{Oid, RepositoryState};

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

/// Which part of a file's unstaged changes a discard threw away, or which part of a commit or
/// comparison's change a revert undid in the working tree (recorded the same way).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discarded {
    File,
    Hunk,
    /// This many selected lines.
    Lines(usize),
    /// A commit or comparison's change reverted in the working tree: a hunk.
    RevertedHunk,
    /// A commit or comparison's change reverted in the working tree: this many selected lines.
    RevertedLines(usize),
    /// A commit or comparison's change reverted in the working tree: the whole change to the file.
    RevertedFile,
}

impl Discarded {
    /// How a stale refusal names the earlier version ("the discarded version is kept as ...").
    pub(super) fn kept_as(self) -> &'static str {
        match self {
            Discarded::File | Discarded::Hunk | Discarded::Lines(_) => "discarded",
            Discarded::RevertedHunk | Discarded::RevertedLines(_) | Discarded::RevertedFile => "overwritten",
        }
    }
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
/// `Resolve` is a conflict resolution saved in the conflict view, or a save with markers.
/// `DeleteFile` is a working-tree file deleted (Delete…, or Unstage and delete… with its
/// `staged` entry), `before` its bytes then and `via` how it went.
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
    /// A deleted local tag. `oid` is the ref's own target: the tag object for an annotated tag.
    /// `remotes` are the remotes the same job also deleted it from, which undo leaves alone.
    DeleteTag {
        name: String,
        oid: Oid,
        remotes: Vec<String>,
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
        replay: Option<Vec<Vec<String>>>,
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
    /// A conflict resolution saved in the conflict view. `ids` are the conflict's stages,
    /// `head` the commit HEAD was at, `before`/`after` the working copy before and after.
    /// `staged` is the entry Save staged; `None` for Save with markers, which left the file
    /// conflicted. A stale refusal drops only this entry.
    Resolve {
        path: String,
        ids: ConflictIds,
        head: Option<Oid>,
        staged: Option<Staged>,
        before: Option<Saved>,
        after: Option<Saved>,
    },
    /// Conflicted files resolved without the editor (Mark resolved, or one side's whole version
    /// taken), or resolved files made conflicted again (Mark conflicted): each changed file's
    /// index entries and working copy before and after, with HEAD at `head` and the repository
    /// in `state`, naming the commits in `heads` (MERGE_HEAD and the like, with what each file
    /// held). Any refusal drops only this entry.
    Conflicts {
        change: ConflictChange,
        head: Option<Oid>,
        state: RepositoryState,
        heads: Vec<(String, String)>,
        files: Vec<ConflictFile>,
    },
    DeleteFile {
        path: String,
        before: Saved,
        via: DeleteVia,
        staged: Option<StagedNew>,
    },
    /// Unstage deleted file: `path`, staged as new (or as a staged rename's new path) and already
    /// gone from the working tree, taken out of the index; `staged` is the entry it had.
    UnstageGone {
        path: String,
        staged: StagedNew,
    },
    /// Branch `name`, not checked out, moved from `from` to `to` (the journal's Restore).
    SetBranch {
        name: String,
        from: Oid,
        to: Oid,
    },
    /// A restack: every branch it moved, `detail` naming it ("restack of the branches above
    /// search"). Also a rebase that moved the branches stacked in it (`rebase.updateRefs`),
    /// `partial`: a branch other than the checked-out one that moved since stays where it is
    /// while the others go back; a restack's undo is refused instead.
    Restacked {
        detail: String,
        moves: Vec<BranchMove>,
        partial: bool,
    },
    /// An abort of a merge, cherry-pick, revert, rebase or patch series (`op`) that kept its
    /// work in the commit `saved` (`aborted.rs`), with HEAD where the abort left it at `head` on
    /// `branch` (`None`: detached), leaving `left` on disk at the operation's paths. `during`:
    /// HEAD while the operation was in progress, when its abort moved HEAD (a rebase, patch
    /// series or run of picks); `None` when the abort left HEAD where it was (a merge, single
    /// pick or revert).
    AbortKept {
        op: MoveOp,
        saved: Oid,
        head: Oid,
        branch: Option<String>,
        left: Changes,
        during: Option<HeadAt>,
    },
    /// Several files' content actions recorded as one entry (`batch.rs`): each part is a
    /// whole-file `Discard` or a `DeleteFile` (an untracked file discarded, or a working copy
    /// deleted), done as `op`.
    Batch {
        op: super::BatchOp,
        parts: Vec<Action>,
    },
    /// Branch `old` renamed to `new` with `git branch -m`; `remote`: the remote the same job also
    /// renamed it on, which undo leaves alone (only named in the label and preview).
    RenameBranch {
        old: String,
        new: String,
        remote: Option<String>,
    },
    /// Remote `old` renamed to `new` (Edit remote). `urls`: the url/pushurl values before and
    /// after when the same edit changed them, put back by undo and set again by redo.
    RenameRemote {
        old: String,
        new: String,
        urls: Option<UrlChange>,
    },
    /// Remote `name` removed; `config` is everything `git remote remove` dropped from the
    /// repository's config, written back by undo.
    RemoveRemote {
        name: String,
        config: RemoteConfig,
    },
    /// A worktree made at `path` with a new branch (`worktree add -b`): `created` is the branch, its
    /// commit and upstream. Undo removes the worktree and deletes the branch while it is unchanged.
    AddWorktree {
        path: std::path::PathBuf,
        created: Created,
    },
}

/// Where HEAD was while an operation was in progress: commit `oid` on `branch` (`None`:
/// detached).
#[derive(Clone, Debug, PartialEq)]
pub struct HeadAt {
    pub branch: Option<String>,
    pub oid: Oid,
}

/// A local branch an operation moved from `from` to `to`.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchMove {
    pub name: String,
    pub from: Oid,
    pub to: Oid,
}

/// A resolution's index entry: its mode and blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Staged {
    pub mode: u32,
    pub blob: Oid,
}

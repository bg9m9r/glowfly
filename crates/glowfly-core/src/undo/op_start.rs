//! Where an operation that can stop for conflicts (merge, pull, cherry-pick, revert, rebase)
//! started: HEAD's branch (or detached commit), its commit and the index's entries. Taken just
//! before Glowfly runs the operation and again after an abort, so the app keeps the undo history
//! only when the abort put all three back exactly (`UndoHistory::aborted`).

use super::HeadPos;
use super::record::head;
use crate::gitcli::args;
use git2::{ObjectType, Oid, Repository, RepositoryState};
use std::path::Path;

/// The repository as an operation found it (opaque to callers; compared whole).
#[derive(Clone, Debug, PartialEq)]
pub struct OpStart {
    head: HeadPos,
    oid: Oid,
    /// A hash over every index entry's path, stage, mode, blob and flags (stat data left out).
    index: Oid,
}

/// The repository's state now, when nothing is in progress and the index has no conflicts;
/// `None` otherwise, or for an unborn HEAD or a repository that can't be read.
pub fn op_start(workdir: &Path) -> Option<OpStart> {
    let repo = Repository::open(workdir).ok()?;
    if repo.state() != RepositoryState::Clean {
        return None;
    }
    let (pos, oid) = head(&repo);
    let index = repo.index().ok()?;
    if index.has_conflicts() {
        return None;
    }
    let mut buf = Vec::new();
    for e in index.iter() {
        buf.extend_from_slice(&e.path);
        buf.push(0);
        buf.extend_from_slice(&e.mode.to_le_bytes());
        buf.extend_from_slice(e.id.as_bytes());
        // The stage (bits 12-13) and the extended flags (intent-to-add, skip-worktree).
        buf.extend_from_slice(&(e.flags & 0x3000).to_le_bytes());
        buf.extend_from_slice(&e.flags_extended.to_le_bytes());
    }
    let index = Oid::hash_object(ObjectType::Blob, &buf).ok()?;
    Some(OpStart { head: pos?, oid: oid?, index })
}

/// Whether an operation (merge, cherry-pick, revert, rebase or patch series) is in progress.
pub fn in_progress(workdir: &Path) -> bool {
    Repository::open(workdir).is_ok_and(|r| r.state() != RepositoryState::Clean)
}

/// Whether `cmds` is a plain abort of a merge, cherry-pick, revert or rebase: one command.
pub fn is_abort(cmds: &[Vec<String>]) -> bool {
    ["merge", "cherry-pick", "revert", "rebase"].iter().any(|op| cmds == [args(&[op, "--abort"])])
}

#[cfg(test)]
mod tests {
    use super::super::tests::commit;
    use super::*;
    use crate::testutil::*;

    #[test]
    fn the_start_changes_with_head_the_branch_and_the_index() {
        let dir = scratch_repo("undo-op-start");
        assert_eq!(op_start(&dir), None, "unborn");
        commit(
            &dir, "a.txt", "one
", "c1",
        );
        let start = op_start(&dir).expect("a clean repository");
        assert_eq!(op_start(&dir), Some(start.clone()), "taken twice, it is the same");
        // A changed working file isn't part of it; a staged one is.
        write(&dir, "a.txt", "changed\n");
        assert_eq!(op_start(&dir), Some(start.clone()));
        git(&dir, &["add", "a.txt"]);
        let staged = op_start(&dir).unwrap();
        assert_ne!(staged, start);
        git(&dir, &["reset", "-q"]);
        assert_eq!(op_start(&dir), Some(start.clone()), "unstaged again, it matches");
        git(&dir, &["update-index", "--skip-worktree", "a.txt"]);
        assert_ne!(op_start(&dir), Some(start.clone()), "a flag counts");
        git(&dir, &["update-index", "--no-skip-worktree", "a.txt"]);
        git(&dir, &["checkout", "-q", "-b", "other"]);
        assert_ne!(op_start(&dir), Some(start.clone()), "another branch at the same commit");
        git(&dir, &["checkout", "-q", "--detach"]);
        assert_ne!(op_start(&dir), Some(start), "detached at the same commit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_is_taken_while_an_operation_is_in_progress() {
        let dir = scratch_repo("undo-op-start-busy");
        commit(
            &dir, "a.txt", "one
", "c1",
        );
        git(&dir, &["checkout", "-q", "-b", "other"]);
        write(&dir, "a.txt", "theirs\n");
        git(&dir, &["commit", "-q", "-am", "theirs"]);
        git(&dir, &["checkout", "-q", "main"]);
        write(&dir, "a.txt", "ours\n");
        git(&dir, &["commit", "-q", "-am", "ours"]);
        assert!(!in_progress(&dir));
        let (ok, _) = crate::gitcli::run(&dir, &args(&["merge", "other"]));
        assert!(!ok, "the merge stops");
        assert!(in_progress(&dir));
        assert_eq!(op_start(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plain_aborts_are_recognised() {
        for op in ["merge", "cherry-pick", "revert", "rebase"] {
            assert!(is_abort(&[args(&[op, "--abort"])]), "{op}");
        }
        assert!(!is_abort(&[args(&["am", "--abort"])]));
        assert!(!is_abort(&[args(&["merge", "--continue"])]));
        assert!(!is_abort(&[args(&["merge", "--abort"]), args(&["status"])]));
    }
}

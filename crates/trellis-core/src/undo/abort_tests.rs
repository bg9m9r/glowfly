//! Aborting a merge while keeping its resolved changes in a stash, and getting them back.

use super::tests::{commit, run_plan};
use super::*;
use crate::gitcli::run;
use crate::repo::status_at;
use crate::testutil::*;
use git2::{Repository, RepositoryState};

#[test]
fn abort_keeping_ends_the_merge_and_undo_restores_the_resolution() {
    let dir = scratch_repo("undo-abort-keep");
    commit(&dir, "a.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "a.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflict");
    write(&dir, "a.txt", "merged\n");
    git(&dir, &["add", "a.txt"]);

    let kind = UndoKind::StashPush { replay: Some(args(&["stash", "push", "-m", ABORT_STASH_MESSAGE])) };
    let b = before(&dir, &kind).unwrap();
    for c in abort_keeping_commands() {
        let (ok, out) = run(&dir, &c);
        assert!(ok, "git {c:?} failed: {out}");
    }
    let action = after(&dir, kind, b).unwrap().expect("the stash push is recorded");

    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.state(), RepositoryState::Clean, "the merge ended");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "ours\n");
    let top = repo.find_commit(repo.refname_to_id("refs/stash").unwrap()).unwrap();
    assert!(top.message().unwrap().contains(ABORT_STASH_MESSAGE), "{:?}", top.message());

    run_plan(&dir, plan(&dir, &action, false).unwrap());
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "merged\n");
    let (_, staged, _) = status_at(&dir, true).unwrap();
    assert!(staged.iter().any(|f| f.path == "a.txt"), "the merged content comes back staged");
}

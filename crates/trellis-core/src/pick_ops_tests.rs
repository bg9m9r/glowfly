//! The cherry-pick and revert command lines, and what git does with them in a fixture.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::{git, scratch_repo, write};
use git2::{Repository, RepositoryState};
use std::path::Path;

fn oid(hex: char) -> Oid {
    Oid::from_str(&hex.to_string().repeat(40)).unwrap()
}

#[test]
fn command_lines() {
    let (a, b) = (oid('a'), oid('b'));
    assert_eq!(cherry_pick_cmd(&[a], None), ["cherry-pick", &a.to_string()]);
    assert_eq!(cherry_pick_cmd(&[a, b], None), ["cherry-pick", &a.to_string(), &b.to_string()]);
    assert_eq!(cherry_pick_cmd(&[a], Some(2)), ["cherry-pick", "-m", "2", &a.to_string()]);
    assert_eq!(revert_cmd(&[b, a], None), ["revert", "--no-edit", &b.to_string(), &a.to_string()]);
    assert_eq!(revert_cmd(&[a], Some(1)), ["revert", "--no-edit", "-m", "1", &a.to_string()]);
    assert_eq!(cherry_pick_cmd(&[a, b], Some(2)), ["cherry-pick", "-m", "2", &a.to_string(), &b.to_string()]);
    assert_eq!(revert_cmd(&[b, a], Some(2)), ["revert", "--no-edit", "-m", "2", &b.to_string(), &a.to_string()]);
    assert_eq!(Picking::CherryPick.skip(), CHERRY_PICK_SKIP);
    assert_eq!(Picking::Revert.skip(), REVERT_SKIP);
    assert_eq!(Picking::CherryPick.abort(), ["cherry-pick", "--abort"]);
    assert_eq!(CHERRY_PICK_SKIP, ["cherry-pick", "--skip"]);
    assert_eq!(REVERT_SKIP, ["revert", "--skip"]);
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

/// main: base, then a merge of `side` (which adds s.txt) made with --no-ff. Returns (dir, the
/// commit before the merge, the merge).
fn merged(name: &str) -> (std::path::PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    let base = commit(&dir, "a.txt", "one\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "s.txt", "side\n", "side work");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    (dir.clone(), base, head(&dir))
}

#[test]
fn a_merge_is_picked_and_reverted_against_its_first_parent() {
    let (dir, base, merge) = merged("pick-mainline");
    git(&dir, &["checkout", "-q", "-b", "other", &base.to_string()]);
    let (ok, out) = run(&dir, &cherry_pick_cmd(&[merge], None));
    assert!(!ok, "a merge needs -m: {out}");
    let (ok, out) = run(&dir, &cherry_pick_cmd(&[merge], Some(1)));
    assert!(ok, "{out}");
    let repo = Repository::open(&dir).unwrap();
    let picked = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(picked.parent_count(), 1, "the pick is an ordinary commit");
    assert_eq!(picked.parent_id(0).unwrap(), base);
    assert!(picked.tree().unwrap().get_name("s.txt").is_some(), "side's file came along");

    git(&dir, &["checkout", "-q", "main"]);
    let (ok, out) = run(&dir, &revert_cmd(&[merge], Some(1)));
    assert!(ok, "{out}");
    assert!(!dir.join("s.txt").exists(), "reverting the merge removed side's file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// main: f.txt "base"; `side` changes f.txt (conflicts with main's edit) then adds g.txt. main
/// then edits f.txt too. Returns (dir, side's conflicting commit, side's clean commit).
fn diverged(name: &str) -> (std::path::PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    commit(&dir, "f.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let clash = commit(&dir, "f.txt", "side\n", "side edit");
    let clean = commit(&dir, "g.txt", "g\n", "add g");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", "main\n", "main edit");
    (dir, clash, clean)
}

fn state(dir: &Path) -> RepositoryState {
    Repository::open(dir).unwrap().state()
}

#[test]
fn skip_moves_a_stopped_cherry_pick_on() {
    let (dir, clash, clean) = diverged("pick-skip");
    let (ok, _) = run(&dir, &cherry_pick_cmd(&[clash, clean], None));
    assert!(!ok, "the first commit conflicts");
    assert!(matches!(state(&dir), RepositoryState::CherryPickSequence | RepositoryState::CherryPick), "{:?}", state(&dir));
    let (ok, out) = run(&dir, &args(CHERRY_PICK_SKIP));
    assert!(ok, "{out}");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "main\n");
    assert!(dir.join("g.txt").exists(), "the second commit was applied");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_moves_a_stopped_revert_on() {
    let dir = scratch_repo("revert-skip");
    commit(&dir, "f.txt", "one\n", "one");
    let first = commit(&dir, "f.txt", "two\n", "two");
    let added = commit(&dir, "g.txt", "g\n", "add g");
    commit(&dir, "f.txt", "three\n", "three");
    // "two" conflicts with "three" and is skipped; g.txt's commit then reverts cleanly.
    let (ok, _) = run(&dir, &revert_cmd(&[first, added], None));
    assert!(!ok, "reverting `two` over `three` conflicts");
    assert!(matches!(state(&dir), RepositoryState::RevertSequence | RepositoryState::Revert), "{:?}", state(&dir));
    let (ok, out) = run(&dir, &args(REVERT_SKIP));
    assert!(ok, "{out}");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert!(!dir.join("g.txt").exists(), "the second revert was applied");
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "three\n");
    let _ = std::fs::remove_dir_all(&dir);
}

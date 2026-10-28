//! The operation a load reports in progress: a pick or revert sequence whose stopped commit was
//! committed (libgit2 says clean, `sequencer/todo` says otherwise), and MERGE_MSG read for a
//! stopped cherry-pick or revert as well as a merge.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{Oid, Repository, RepositoryState};
use std::path::{Path, PathBuf};

/// base (a.txt "one"); `other` with q1 (a.txt) and r (adds c.txt); main with an edit to a.txt
/// that q1 conflicts with. Returns (dir, [q1, r]).
fn picks(name: &str) -> (PathBuf, [Oid; 2]) {
    let dir = scratch_repo(name);
    let commit = |file: &str, text: &str, msg: &str| {
        write(&dir, file, text);
        git(&dir, &["add", file]);
        git(&dir, &["commit", "-q", "-m", msg]);
        Repository::open(&dir).unwrap().head().unwrap().target().unwrap()
    };
    commit("a.txt", "one\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    let q1 = commit("a.txt", "one\nq1\n", "q1");
    let r = commit("c.txt", "c\n", "r");
    git(&dir, &["checkout", "-q", "main"]);
    commit("a.txt", "one\nmain\n", "main edit");
    (dir, [q1, r])
}

fn stops(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(!ok, "git {a:?} should stop for conflicts: {out}");
}

fn loaded(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(10)).unwrap()
}

fn libgit2_state(dir: &Path) -> RepositoryState {
    Repository::open(dir).unwrap().state()
}

#[test]
fn a_pick_sequence_whose_stopped_commit_was_committed_stays_in_progress() {
    let (dir, [q1, r]) = picks("op-state-pick-seq");
    stops(&dir, &["cherry-pick", &q1.to_string(), &r.to_string()]);
    assert_eq!(loaded(&dir).state, RepositoryState::CherryPickSequence);
    write(&dir, "a.txt", "one\nmain\nq1\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "--no-edit"]);
    assert_eq!(libgit2_state(&dir), RepositoryState::Clean, "git removed CHERRY_PICK_HEAD");
    assert_eq!(loaded(&dir).state, RepositoryState::CherryPickSequence, "the rest of the sequence is still to run");
    assert_eq!(status_at(&dir, false).unwrap().2, RepositoryState::CherryPickSequence, "a status read agrees");

    git(&dir, &["cherry-pick", "--continue"]);
    assert_eq!(loaded(&dir).state, RepositoryState::Clean);
    assert!(dir.join("c.txt").is_file(), "r was picked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_sequence_whose_stopped_commit_was_committed_stays_in_progress() {
    let dir = scratch_repo("op-state-revert-seq");
    let commit = |file: &str, text: &str, msg: &str| {
        write(&dir, file, text);
        git(&dir, &["add", file]);
        git(&dir, &["commit", "-q", "-m", msg]);
        Repository::open(&dir).unwrap().head().unwrap().target().unwrap()
    };
    commit("a.txt", "one\n", "base");
    let c = commit("c.txt", "c\n", "add c");
    let a = commit("a.txt", "one\ntwo\n", "two");
    commit("a.txt", "one\ntwo\nthree\n", "three");
    stops(&dir, &["revert", "--no-edit", &a.to_string(), &c.to_string()]);
    assert_eq!(loaded(&dir).state, RepositoryState::RevertSequence);
    write(&dir, "a.txt", "one\nthree\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "--no-edit"]);
    assert_eq!(libgit2_state(&dir), RepositoryState::Clean, "git removed REVERT_HEAD");
    assert_eq!(loaded(&dir).state, RepositoryState::RevertSequence);

    git(&dir, &["revert", "--continue"]);
    assert_eq!(loaded(&dir).state, RepositoryState::Clean);
    assert!(!dir.join("c.txt").exists(), "add c was reverted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sequencer_folder_without_picks_left_is_clean() {
    let (dir, _) = picks("op-state-stale");
    let seq = dir.join(".git/sequencer");
    std::fs::create_dir_all(&seq).unwrap();
    assert_eq!(loaded(&dir).state, RepositoryState::Clean, "no todo");
    std::fs::write(seq.join("todo"), "# nothing left\n\n").unwrap();
    assert_eq!(loaded(&dir).state, RepositoryState::Clean, "only comments");
    std::fs::write(seq.join("todo"), "exec make\n").unwrap();
    assert_eq!(loaded(&dir).state, RepositoryState::Clean, "not a pick or revert");
    std::fs::write(seq.join("todo"), "# picks\npick 1234567 q1\n").unwrap();
    assert_eq!(loaded(&dir).state, RepositoryState::CherryPickSequence);
    std::fs::write(seq.join("todo"), "revert 1234567 q1\n").unwrap();
    assert_eq!(loaded(&dir).state, RepositoryState::RevertSequence);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_cherry_pick_or_revert_reads_its_message() {
    let (dir, [q1, _]) = picks("op-state-pick-msg");
    stops(&dir, &["cherry-pick", &q1.to_string()]);
    let snap = loaded(&dir);
    assert_eq!(snap.state, RepositoryState::CherryPick);
    assert_eq!(snap.merge_message.as_deref(), Some("q1"), "without git's conflict comments");
    git(&dir, &["cherry-pick", "--abort"]);
    assert_eq!(loaded(&dir).merge_message, None);

    git(&dir, &["checkout", "-q", "other"]);
    git(&dir, &["reset", "-q", "--hard", &q1.to_string()]);
    write(&dir, "a.txt", "one\nq1\nmore\n");
    git(&dir, &["commit", "-q", "-am", "more"]);
    stops(&dir, &["revert", "--no-edit", &q1.to_string()]);
    let snap = loaded(&dir);
    assert_eq!(snap.state, RepositoryState::Revert);
    let message = snap.merge_message.expect("the revert's message");
    assert!(message.starts_with("Revert \"q1\"") && message.contains(&q1.to_string()), "{message:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

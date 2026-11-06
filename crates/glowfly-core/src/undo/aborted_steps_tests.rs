//! Aborting a multi-commit cherry-pick or revert, or a patch series (`git am`), while keeping its
//! work, and Undo putting the branch back on the commits applied so far with the operation in
//! progress; unrelated edits staying through both; and the refusals: HEAD moved since it
//! stopped (abort-safety, dirtyindex) before the abort, and HEAD moved or a new operation
//! started before the undo.

use super::aborted_state_tests::{folder, head_at, ok_git, staging_left};
use super::aborted_tests::{abort, git_file, kept_refs, read, staged, state, undo};
use super::tests::commit;
use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use git2::{Oid, RepositoryState};
use std::path::{Path, PathBuf};

fn subjects(dir: &Path) -> Vec<String> {
    ok_git(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

/// main has f.txt and README.md; feature adds g.txt (x1), then changes f.txt (x2); main changes
/// f.txt too. Stopped cherry-picking x1 and x2 onto main, at x2. Returns main's tip before.
fn stopped_picks(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "README.md", "readme\n");
    commit(&dir, "f.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    let x1 = commit(&dir, "g.txt", "g\n", "x1");
    let x2 = commit(&dir, "f.txt", "feature\n", "x2");
    git(&dir, &["checkout", "-q", "main"]);
    let h0 = commit(&dir, "f.txt", "main\n", "main edit");
    let (ok, out) = run(&dir, &args(&["cherry-pick", &x1.to_string(), &x2.to_string()]));
    assert!(!ok, "the picks stop at x2: {out}");
    assert_eq!(state(&dir), RepositoryState::CherryPickSequence);
    (dir, h0)
}

#[test]
fn cherry_pick_sequence_abort_kept_comes_back() {
    let (dir, h0) = stopped_picks("abort-steps-picks");
    write(&dir, "f.txt", "resolved\n");
    git(&dir, &["add", "f.txt"]);
    let (stop, before, pick_head) = (head_at(&dir), folder(&dir, "sequencer"), git_file(&dir, "CHERRY_PICK_HEAD"));
    assert_eq!(stop.0.as_deref(), Some("main"), "picks commit on the branch");

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of cherry-pick");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(head_at(&dir), (Some("main".into()), h0));
    assert!(!dir.join("g.txt").exists(), "x1's pick goes with the branch");
    assert!(!dir.join(".git/sequencer").exists());

    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::CherryPickSequence);
    assert_eq!(head_at(&dir), stop, "main back on the picks so far");
    assert_eq!(folder(&dir, "sequencer"), before);
    assert_eq!(git_file(&dir, "CHERRY_PICK_HEAD"), pick_head);
    assert_eq!((read(&dir, "f.txt"), read(&dir, "g.txt")), ("resolved\n".into(), "g\n".into()));
    assert!(staged(&dir).iter().any(|(p, _, _)| p == "f.txt"));
    assert!(!staging_left(&dir));

    // Redo aborts again: git's reset takes the entries put back as unchanged.
    assert_eq!(plan(&dir, &action, true).unwrap(), Plan::AbortAgain);
    let again = abort(&dir);
    assert_eq!(head_at(&dir), (Some("main".into()), h0));
    undo(&dir, &again);
    ok_git(&dir, &["cherry-pick", "--continue"]);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(subjects(&dir), ["x2", "x1", "main edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_sequence_abort_kept_comes_back() {
    let dir = scratch_repo("abort-steps-reverts");
    commit(&dir, "f.txt", "one\n", "base");
    let r1 = commit(&dir, "f.txt", "two\n", "r1");
    let r2 = commit(&dir, "g.txt", "g\n", "r2");
    let h0 = commit(&dir, "f.txt", "three\n", "r3");
    let (ok, out) = run(&dir, &args(&["revert", "--no-edit", &r2.to_string(), &r1.to_string()]));
    assert!(!ok, "reverting r1 conflicts: {out}");
    assert_eq!(state(&dir), RepositoryState::RevertSequence);
    write(&dir, "f.txt", "one and three\n");
    git(&dir, &["add", "f.txt"]);
    let (stop, before, revert_head) = (head_at(&dir), folder(&dir, "sequencer"), git_file(&dir, "REVERT_HEAD"));

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of revert");
    assert_eq!(head_at(&dir), (Some("main".into()), h0));
    assert_eq!(read(&dir, "g.txt"), "g\n", "r2's revert goes");
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::RevertSequence);
    assert_eq!((head_at(&dir), folder(&dir, "sequencer"), git_file(&dir, "REVERT_HEAD")), (stop, before, revert_head));
    assert!(!dir.join("g.txt").exists());
    ok_git(&dir, &["revert", "--continue"]);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(subjects(&dir).len(), 6);
    assert_eq!(read(&dir, "f.txt"), "one and three\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// a.txt and README.md; p1 adds b.txt, p2 changes a.txt, saved as a series; then the branch is
/// back at base with "mine" changing a.txt. Stopped applying the series with `--3way`, at p2.
/// Returns "mine" and the series' file.
fn stopped_am(name: &str) -> (PathBuf, Oid, PathBuf) {
    let dir = scratch_repo(name);
    write(&dir, "README.md", "readme\n");
    commit(&dir, "a.txt", "base\n", "base");
    let p1 = commit(&dir, "b.txt", "b\n", "p1");
    let p2 = commit(&dir, "a.txt", "theirs\n", "p2");
    let series = crate::paths::temp_dir().join(format!("glowfly-test-{name}-{}.patch", std::process::id()));
    crate::patch_ops::export(&dir, &[p1, p2], &series).unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~2"]);
    let mine = commit(&dir, "a.txt", "mine\n", "mine");
    let (ok, out) = run(&dir, &args(&["am", "--3way", &series.to_string_lossy()]));
    assert!(!ok, "p2 conflicts: {out}");
    assert_eq!(state(&dir), RepositoryState::ApplyMailbox);
    (dir, mine, series)
}

#[test]
fn am_series_abort_kept_comes_back() {
    let (dir, mine, series) = stopped_am("abort-steps-am");
    write(&dir, "a.txt", "mine and theirs\n");
    git(&dir, &["add", "a.txt"]);
    let (stop, before) = (head_at(&dir), folder(&dir, "rebase-apply"));
    assert!(before.contains_key("rebase-apply/0002") && before.contains_key("rebase-apply/applying"), "{:?}", before.keys());

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of patch series");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(head_at(&dir), (Some("main".into()), mine));
    assert!(!dir.join(".git/rebase-apply").exists());
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::ApplyMailbox);
    assert_eq!(head_at(&dir), stop, "the branch back on p1");
    assert_eq!(folder(&dir, "rebase-apply"), before);
    assert_eq!(read(&dir, "a.txt"), "mine and theirs\n");
    assert!(staged(&dir).iter().any(|(p, _, _)| p == "a.txt"));
    ok_git(&dir, &["am", "--continue"]);
    assert_eq!(subjects(&dir), ["p2", "p1", "mine", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&series);
}

#[test]
fn unrelated_edits_stay_through_kept_sequence_and_am_aborts() {
    // git's own abort of both is a reset that keeps unstaged edits to files it doesn't touch, so
    // they aren't kept; if a git version drops them, the op needs `resets_all`.
    let (dir, _) = stopped_picks("abort-steps-picks-unrelated");
    write(&dir, "README.md", "readme, mine\n");
    let action = abort(&dir);
    assert_eq!(read(&dir, "README.md"), "readme, mine\n");
    undo(&dir, &action);
    assert_eq!(read(&dir, "README.md"), "readme, mine\n");
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, _, series) = stopped_am("abort-steps-am-unrelated");
    write(&dir, "README.md", "readme, mine\n");
    let action = abort(&dir);
    assert_eq!(read(&dir, "README.md"), "readme, mine\n");
    undo(&dir, &action);
    assert_eq!(read(&dir, "README.md"), "readme, mine\n");
    assert_eq!(state(&dir), RepositoryState::ApplyMailbox);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&series);
}

/// Aborting keeping the work is refused with nothing changed.
fn refused(dir: &Path, why: &str) {
    let (at, s) = (head_at(dir), state(dir));
    let r = abort_keeping(dir, Some(&[]), |_| panic!("nothing runs"));
    assert!(!r.ok && r.output.starts_with(NOTHING_ABORTED) && r.output.contains(why), "{}", r.output);
    assert_eq!((head_at(dir), state(dir), kept_refs(dir)), (at, s, 0));
}

#[test]
fn abort_safety_mismatch_refuses() {
    let (dir, h0) = stopped_picks("abort-steps-safety");
    write(&dir, ".git/sequencer/abort-safety", &format!("{h0}\n"));
    refused(&dir, "HEAD has moved since the cherry-pick stopped");
    std::fs::remove_file(dir.join(".git/sequencer/abort-safety")).unwrap();
    refused(&dir, "HEAD has moved since the cherry-pick stopped");
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, _, series) = stopped_am("abort-steps-dirtyindex");
    write(&dir, ".git/rebase-apply/dirtyindex", "");
    refused(&dir, "started with staged changes");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&series);
}

#[test]
fn undo_refused_when_head_moved_or_a_new_operation_started() {
    let (dir, _) = stopped_picks("abort-steps-undo-refused");
    write(&dir, "f.txt", "resolved\n");
    git(&dir, &["add", "f.txt"]);
    let action = abort(&dir);

    // An edit at one of its paths: the entry stays.
    write(&dir, "f.txt", "new work\n");
    assert!(matches!(plan(&dir, &action, false).unwrap(), Plan::Refuse(Refusal::Dirty(_))));
    git(&dir, &["checkout", "-q", "--", "f.txt"]);
    assert_eq!(plan(&dir, &action, false).unwrap(), Plan::RestoreAbort);

    // A sequencer folder again, even one libgit2 calls no operation.
    std::fs::create_dir(dir.join(".git/sequencer")).unwrap();
    assert_eq!(state(&dir), RepositoryState::Clean);
    let Plan::Refuse(r) = plan(&dir, &action, false).unwrap() else { panic!("a new operation") };
    assert!(r.is_stale() && r.message().contains("was started since") && r.message().contains(ABORT_REF_PREFIX), "{r:?}");
    std::fs::remove_dir(dir.join(".git/sequencer")).unwrap();

    commit(&dir, "u.txt", "later\n", "later");
    let Plan::Refuse(r) = plan(&dir, &action, false).unwrap() else { panic!("HEAD moved") };
    assert!(r.is_stale() && r.message().contains("HEAD has moved") && r.message().contains(ABORT_REF_PREFIX), "{r:?}");
    assert_eq!(kept_refs(&dir), 1, "kept for recovery");
    assert!(!dir.join(".git/sequencer").exists(), "nothing written");
    let _ = std::fs::remove_dir_all(&dir);
}

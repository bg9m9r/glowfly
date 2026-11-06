//! Aborting a rebase while keeping its work, and Undo putting it back in progress at the same
//! step: its state folder byte for byte, HEAD detached where it stopped, REBASE_HEAD, its label
//! refs, the resolutions staged and the unrelated edits its hard reset drops; Redo aborting it
//! again; a rebase with an autostash refused; and a failed abort put back.

use super::aborted_tests::{abort, git_file, kept_refs, read, run_all, staged, state, two_conflicts, undo};
use super::tests::commit;
use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use git2::{Repository, RepositoryState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file under the git folder's `name`, by path, with its bytes.
pub(super) fn folder(dir: &Path, name: &str) -> BTreeMap<String, Vec<u8>> {
    fn walk(path: &Path, rel: &str, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(path).unwrap() {
            let e = e.unwrap();
            let rel = format!("{rel}/{}", e.file_name().to_string_lossy());
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), &rel, out);
            } else {
                out.insert(rel, std::fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&dir.join(".git").join(name), name, &mut out);
    assert!(!out.is_empty(), "{name} holds files");
    out
}

/// HEAD's branch (`None`: detached) and commit.
pub(super) fn head_at(dir: &Path) -> (Option<String>, git2::Oid) {
    let repo = Repository::open(dir).unwrap();
    let h = repo.head().unwrap();
    (h.is_branch().then(|| h.shorthand().unwrap().to_owned()), h.target().unwrap())
}

/// Whether a staging folder of a restore was left in the git folder.
pub(super) fn staging_left(dir: &Path) -> bool {
    std::fs::read_dir(dir.join(".git")).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("glowfly-restoring-"))
}

pub(super) fn ok_git(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?}: {out}");
    out
}

/// main: README.md and a.txt; feature adds b.txt (f1), then changes a.txt (f2); main changes
/// a.txt too. Stopped rebasing feature onto main with `how`, at f2. Returns feature's old tip.
fn stopped(name: &str, how: &[&str]) -> (PathBuf, git2::Oid) {
    let dir = scratch_repo(name);
    write(&dir, "README.md", "readme\n");
    commit(&dir, "a.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "b.txt", "b\n", "f1");
    let tip = commit(&dir, "a.txt", "feature\n", "f2");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    git(&dir, &["checkout", "-q", "feature"]);
    let mut cmd = vec!["rebase"];
    cmd.extend(how);
    cmd.push("main");
    let (ok, out) = run(&dir, &args(&cmd));
    assert!(!ok, "the rebase stops at f2: {out}");
    (dir, tip)
}

/// A rebase of the merge backend is in progress (with or without its `interactive` marker).
fn rebasing(dir: &Path) -> bool {
    matches!(state(dir), RepositoryState::RebaseMerge | RepositoryState::RebaseInteractive)
}

fn log(dir: &Path) -> Vec<String> {
    ok_git(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

#[test]
fn rebase_abort_kept_comes_back_at_the_same_step() {
    let (dir, tip) = stopped("abort-steps-rebase", &[]);
    let was = state(&dir);
    assert!(rebasing(&dir), "{was:?}");
    write(&dir, "a.txt", "resolved\n");
    git(&dir, &["add", "a.txt"]);
    let (on, s) = head_at(&dir);
    assert_eq!(on, None, "a rebase runs detached");
    let (before, rebase_head, orig_head) = (folder(&dir, "rebase-merge"), git_file(&dir, "REBASE_HEAD"), git_file(&dir, "ORIG_HEAD"));
    assert!(rebase_head.is_some());

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of rebase");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(head_at(&dir), (Some("feature".into()), tip), "back on the branch at its old tip");
    assert_eq!(read(&dir, "a.txt"), "feature\n");
    assert!(!dir.join(".git/rebase-merge").exists());
    assert_eq!(kept_refs(&dir), 1);

    undo(&dir, &action);
    assert_eq!(state(&dir), was);
    assert_eq!(head_at(&dir), (None, s));
    assert_eq!(folder(&dir, "rebase-merge"), before, "the state folder byte for byte");
    assert_eq!((git_file(&dir, "REBASE_HEAD"), git_file(&dir, "ORIG_HEAD")), (rebase_head, orig_head));
    assert_eq!(read(&dir, "a.txt"), "resolved\n");
    assert!(staged(&dir).iter().any(|(p, s, _)| p == "a.txt" && *s == 'M'), "{:?}", staged(&dir));
    assert_eq!(kept_refs(&dir), 0);
    assert!(!staging_left(&dir));

    ok_git(&dir, &["rebase", "--continue"]);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(log(&dir), ["f2", "f1", "main edit", "base"]);
    assert_eq!(head_at(&dir).0.as_deref(), Some("feature"));
    assert_eq!(read(&dir, "a.txt"), "resolved\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_redo_aborts_again() {
    let (dir, tip) = stopped("abort-steps-redo", &[]);
    write(&dir, "a.txt", "resolved\n");
    git(&dir, &["add", "a.txt"]);
    let (_, s) = head_at(&dir);
    let action = abort(&dir);
    assert_eq!(
        plan(&dir, &action, true).unwrap(),
        Plan::Refuse(Refusal::Stale("The rebase is no longer in progress; undo history cleared".into()))
    );
    undo(&dir, &action);
    assert_eq!(plan(&dir, &action, true).unwrap(), Plan::AbortAgain);
    let again = abort(&dir);
    assert_eq!(head_at(&dir), (Some("feature".into()), tip));
    let Action::AbortKept { during, .. } = &again else { panic!("a kept abort") };
    assert_eq!(during, &Some(HeadAt { branch: None, oid: s }));
    undo(&dir, &again);
    assert_eq!(read(&dir, "a.txt"), "resolved\n");
    assert!(rebasing(&dir), "{:?}", state(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_merges_labels_come_back() {
    let (dir, _) = stopped("abort-steps-labels", &["-r"]);
    assert!(rebasing(&dir), "{:?}", state(&dir));
    let label = |dir: &Path| Repository::open(dir).unwrap().refname_to_id("refs/rewritten/onto").ok();
    let onto = label(&dir).expect("the label exists while stopped");
    write(&dir, "a.txt", "resolved\n");
    git(&dir, &["add", "a.txt"]);
    let action = abort(&dir);
    assert_eq!(label(&dir), None, "the abort deletes it");
    undo(&dir, &action);
    assert_eq!(label(&dir), Some(onto));
    ok_git(&dir, &["rebase", "--continue"]);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(log(&dir), ["f2", "f1", "main edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn interactive_rebase_keeps_its_edited_todo() {
    let dir = scratch_repo("abort-steps-edit");
    commit(&dir, "a.txt", "one\n", "c1");
    let c2 = commit(&dir, "a.txt", "two\n", "c2");
    commit(&dir, "a.txt", "three\n", "c3");
    commit(&dir, "a.txt", "four\n", "c4");
    let p = crate::rebase::plan(&dir, &crate::rebase::RebaseBase::FromCommit(c2)).unwrap();
    let mut items: Vec<_> = p.steps.into_iter().filter_map(|s| s.commit().cloned()).collect();
    items[1].action = crate::rebase::RebaseAction::Edit;
    let items = items.into_iter().map(crate::rebase::TodoStep::Commit).collect();
    let req = crate::rebase::StartRequest { head: p.head, upstream: p.upstream, items, autostash: false };
    crate::rebase::start(&dir, &req, &crate::rebase::SequenceEditor::Copy).unwrap();
    assert_eq!(state(&dir), RepositoryState::RebaseInteractive);
    let stop = head_at(&dir);
    let (todo, done) = (git_file(&dir, "rebase-merge/git-rebase-todo"), git_file(&dir, "rebase-merge/done"));
    assert!(todo.is_some() && done.is_some());

    let action = abort(&dir);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(read(&dir, "a.txt"), "four\n");
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::RebaseInteractive);
    assert_eq!((git_file(&dir, "rebase-merge/git-rebase-todo"), git_file(&dir, "rebase-merge/done")), (todo, done));
    assert_eq!(head_at(&dir), stop, "at the edit stop");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_unrelated_edit_comes_back() {
    let (dir, _) = stopped("abort-steps-unrelated", &[]);
    write(&dir, "README.md", "readme, mine\n");
    let action = abort(&dir);
    assert_eq!(read(&dir, "README.md"), "readme\n", "git's hard reset drops it");
    undo(&dir, &action);
    assert_eq!(read(&dir, "README.md"), "readme, mine\n");
    assert!(rebasing(&dir), "{:?}", state(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_with_autostash_is_refused_with_nothing_changed() {
    let dir = scratch_repo("abort-steps-autostash");
    write(&dir, "README.md", "readme\n");
    commit(&dir, "a.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "a.txt", "feature\n", "f1");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    git(&dir, &["checkout", "-q", "feature"]);
    write(&dir, "README.md", "readme, dirty\n");
    let (ok, _) = run(&dir, &args(&["rebase", "--autostash", "main"]));
    assert!(!ok, "the rebase stops");
    assert!(dir.join(".git/rebase-merge/autostash").exists());
    let at = head_at(&dir);
    let r = abort_keeping(&dir, Some(&[]), |_| panic!("nothing runs"));
    assert!(!r.ok && r.action.is_none());
    assert!(r.output.starts_with(NOTHING_ABORTED) && r.output.contains("stashed your changes"), "{}", r.output);
    assert!(rebasing(&dir), "{:?}", state(&dir));
    assert_eq!(head_at(&dir), at);
    assert_eq!(kept_refs(&dir), 0);
    assert!(dir.join(".git/rebase-merge/autostash").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_backend_rebase_is_kept() {
    let (dir, tip) = stopped("abort-steps-apply", &["--apply"]);
    assert_eq!(state(&dir), RepositoryState::Rebase);
    assert!(dir.join(".git/rebase-apply/rebasing").exists());
    write(&dir, "a.txt", "resolved\n");
    git(&dir, &["add", "a.txt"]);
    let (stop, before) = (head_at(&dir), folder(&dir, "rebase-apply"));
    let action = abort(&dir);
    assert_eq!(head_at(&dir), (Some("feature".into()), tip));
    assert!(!dir.join(".git/rebase-apply").exists());
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::Rebase);
    assert_eq!(head_at(&dir), stop);
    assert_eq!(folder(&dir, "rebase-apply"), before);
    ok_git(&dir, &["rebase", "--continue"]);
    assert_eq!(log(&dir), ["f2", "f1", "main edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_steps_abort_is_put_back() {
    let (dir, _) = stopped("abort-steps-failed", &[]);
    write(&dir, "a.txt", "resolved\n");
    git(&dir, &["add", "a.txt"]);
    let (stop, before) = (head_at(&dir), folder(&dir, "rebase-merge"));
    // Nothing ran.
    let r = abort_keeping(&dir, Some(&[]), |_| (false, "boom".into()));
    assert!(!r.ok && r.output.contains("everything it had was put back"), "{}", r.output);
    assert!(rebasing(&dir), "{:?}", state(&dir));
    assert_eq!((head_at(&dir), folder(&dir, "rebase-merge")), (stop.clone(), before.clone()));
    assert_eq!(kept_refs(&dir), 0);
    // The abort ran whole, then said it failed: everything comes back from Clean.
    let r = abort_keeping(&dir, Some(&[]), |cmds| {
        let (ok, out) = run_all(&dir, cmds);
        assert!(ok, "{out}");
        (false, "error: unable to unlink a.txt".into())
    });
    assert!(!r.ok && r.output.contains("everything it had was put back"), "{}", r.output);
    assert!(rebasing(&dir), "{:?}", state(&dir));
    assert_eq!((head_at(&dir), folder(&dir, "rebase-merge")), (stop, before));
    assert_eq!(read(&dir, "a.txt"), "resolved\n");
    assert!(!staging_left(&dir));
    assert!(!dir.join(".git/rebase-merge.bak").exists());
    ok_git(&dir, &["rebase", "--continue"]);
    assert_eq!(log(&dir), ["f2", "f1", "main edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merges_kept_tree_has_no_state_folder() {
    let dir = two_conflicts("abort-steps-merge-tree");
    let action = abort(&dir);
    let Action::AbortKept { saved, during, .. } = &action else { panic!("a kept abort") };
    assert_eq!(during, &None);
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.find_commit(*saved).unwrap().tree().unwrap();
    assert!(tree.get_name("gitdir").is_none() && tree.get_name("heads").is_none());
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::Merge);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_new_ops_are_named() {
    assert_eq!(MoveOp::Am.noun(), "patch series");
    let o = git2::Oid::zero();
    let kept = |op| Action::AbortKept { op, saved: o, head: o, branch: None, left: Changes(Vec::new()), during: None };
    assert_eq!(kept(MoveOp::Am).describe(), "abort of patch series");
    assert_eq!(kept(MoveOp::Rebase).describe(), "abort of rebase");
    for (s, want) in [
        (RepositoryState::CherryPickSequence, Some(("sequencer", "cherry-pick"))),
        (RepositoryState::RevertSequence, Some(("sequencer", "revert"))),
        (RepositoryState::RebaseMerge, Some(("rebase-merge", "rebase"))),
        (RepositoryState::RebaseInteractive, Some(("rebase-merge", "rebase"))),
        (RepositoryState::Rebase, Some(("rebase-apply", "rebase"))),
        (RepositoryState::ApplyMailbox, Some(("rebase-apply", "am"))),
        (RepositoryState::ApplyMailboxOrRebase, None),
        (RepositoryState::Bisect, None),
        (RepositoryState::Merge, None),
    ] {
        let got = match aborted::aborting(s) {
            Some(aborted::Aborting::Steps { dir, verb, .. }) => Some((dir, verb)),
            _ => None,
        };
        assert_eq!(got, want, "{s:?}");
    }
}

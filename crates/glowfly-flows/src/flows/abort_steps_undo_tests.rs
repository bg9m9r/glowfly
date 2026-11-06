//! The banner's Abort of a run of steps (a rebase, a multi-commit cherry-pick, a patch series)
//! keeping its work: after a resolved conflict, Undo puts it back in progress at the same step
//! with the resolution staged, Redo aborts it again, and Continue then finishes it. A rebase
//! that stashed changes itself aborts plainly.

use super::op_progress_tests::{ask, banner, opened, read, repo_state, resolve, staged_paths, undo_top};
use super::patch_tests::{beside, cleanup, commit, last_toast, three_commits};
use super::selection_tests::commit_on;
use super::tests::{drive, git, scratch_repo};
use git2::{Oid, Repository, RepositoryState};
use glowfly_core::gitcli::{args, run};
use glowfly_core::i18n::Lang;
use glowfly_core::patch_ops::{self, AM_CONTINUE, AmOptions};
use glowfly_core::rebase::Resume;
use glowfly_core::undo::Action;
use glowfly_model::msg::{JobMsg, Msg, RebaseMsg, RepoMsg, SelectionMsg};
use glowfly_model::patch::{ApplyHow, ApplyRequest, PatchMsg};
use glowfly_model::state::op_actions::banner_title;
use glowfly_model::state::{AppState, Modal};
use std::path::{Path, PathBuf};

/// HEAD's branch (`None`: detached) and commit.
fn head_of(dir: &Path) -> (Option<String>, Oid) {
    let repo = Repository::open(dir).unwrap();
    let h = repo.head().unwrap();
    (h.is_branch().then(|| h.shorthand().unwrap().to_owned()), h.target().unwrap())
}

fn subjects(dir: &Path, rev: &str) -> Vec<String> {
    let (ok, out) = run(dir, &args(&["log", "--format=%s", rev]));
    assert!(ok, "{out}");
    out.lines().map(str::to_owned).collect()
}

fn title(st: &AppState) -> Option<String> {
    banner_title(Lang::En, st.snap.as_ref().unwrap())
}

fn redo_label(st: &AppState) -> String {
    st.session.undo.redo_entry(Lang::En, true).0
}

fn rebasing(st: &AppState) -> bool {
    matches!(repo_state(st), RepositoryState::RebaseMerge | RepositoryState::RebaseInteractive)
}

/// feature adds b.txt (f1), then changes a.txt (f2); main changes a.txt too. Stopped rebasing
/// feature onto main at f2 (with `how`), then opened. Returns feature's old tip.
fn stopped_rebase(name: &str, how: &[&str]) -> (AppState, PathBuf, Oid) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("README.md"), "readme\n").unwrap();
    commit_on(&dir, "readme", 1);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_on(&dir, "f1", 2);
    std::fs::write(dir.join("a.txt"), "one\nfeature\n").unwrap();
    let tip = commit_on(&dir, "f2", 3);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nmain\n").unwrap();
    commit_on(&dir, "main edit", 4);
    git(&dir, &["checkout", "-q", "feature"]);
    if how.contains(&"--autostash") {
        std::fs::write(dir.join("README.md"), "readme, dirty\n").unwrap();
    }
    let mut cmd = vec!["rebase"];
    cmd.extend(how);
    cmd.push("main");
    let (ok, out) = run(&dir, &args(&cmd));
    assert!(!ok, "the rebase stops at f2: {out}");
    (opened(&dir), dir, tip)
}

#[test]
fn rebase_abort_after_a_resolved_conflict_undo_and_redo() {
    let (mut st, dir, tip) = stopped_rebase("abort-steps-rebase-flow", &[]);
    assert!(rebasing(&st), "{:?}", repo_state(&st));
    resolve(&mut st, &dir, "a.txt", "one\nresolved\n");
    let (stop, at_step) = (head_of(&dir), title(&st));
    assert!(at_step.as_deref().is_some_and(|t| t.ends_with("ebase in progress \u{b7} step 2 of 2")), "{at_step:?}");
    let Msg::ConfirmDanger { body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert!(body.contains("Glowfly keeps them first"), "{body}");
    assert!(!body.contains("Your other changes stay"), "{body}");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir), (Some("feature".into()), tip));
    assert!(matches!(undo_top(&st).0, Some(Action::AbortKept { .. })));
    assert_eq!(undo_top(&st).1, "Undo abort of rebase");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(rebasing(&st), "in progress again: {:?}", repo_state(&st));
    assert_eq!(head_of(&dir), stop, "detached where it stopped");
    assert_eq!(read(&dir, "a.txt"), "one\nresolved\n");
    assert_eq!(staged_paths(&st), ["a.txt"]);
    assert_eq!(title(&st), at_step, "the banner shows the same step");
    assert_eq!(redo_label(&st), "Redo abort of rebase");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir), (Some("feature".into()), tip));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(rebasing(&st));
    drive(&mut st, Msg::Rebase(RebaseMsg::Resume(Resume::Continue)));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(subjects(&dir, "feature"), ["f2", "f1", "main edit", "readme", "first"]);
    assert_eq!(read(&dir, "a.txt"), "one\nresolved\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_pick_sequence_abort_after_a_resolved_conflict_undo_and_redo() {
    let dir = scratch_repo("abort-steps-pick-flow");
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("f.txt"), "base\n").unwrap();
    commit_on(&dir, "base", 1);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("g.txt"), "g\n").unwrap();
    let x1 = commit_on(&dir, "x1", 2);
    std::fs::write(dir.join("f.txt"), "feature\n").unwrap();
    let x2 = commit_on(&dir, "x2", 3);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("f.txt"), "main\n").unwrap();
    let h0 = commit_on(&dir, "main edit", 4);
    let mut st = opened(&dir);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![x1, x2])));
    assert_eq!(repo_state(&st), RepositoryState::CherryPickSequence);
    resolve(&mut st, &dir, "f.txt", "resolved\n");
    let stop = head_of(&dir);

    ask(&mut st, "Abort");
    let Some(Modal::ConfirmDanger { title, body, .. }) = &st.modal else { panic!("asks first") };
    assert_eq!(title, "Abort the cherry-pick?");
    assert!(body.contains("Glowfly keeps them first"), "{body}");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir), (Some("main".into()), h0));
    assert_eq!(undo_top(&st).1, "Undo abort of cherry-pick");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::CherryPickSequence);
    assert_eq!(head_of(&dir), stop, "main back on x1's pick");
    assert!(dir.join(".git/CHERRY_PICK_HEAD").exists() && dir.join(".git/sequencer/todo").exists());
    assert_eq!(staged_paths(&st), ["f.txt"]);
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir).1, h0);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    drive(&mut st, Msg::git("Continue", &["cherry-pick", "--continue"]));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(subjects(&dir, "main")[..3], ["x2", "x1", "main edit"]);
    assert_eq!(read(&dir, "f.txt"), "resolved\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn patch_series_abort_after_a_resolved_conflict_undo_and_redo() {
    // A series adding b.txt (p1) and changing a.txt (p2), applied onto "Mine", which changes
    // a.txt too.
    let (mut st, dir) = three_commits("abort-steps-am-flow");
    std::fs::write(dir.join("a.txt"), "one\ntwo\nfour\n").unwrap();
    commit_on(&dir, "Fourth", 9);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let p = beside(&dir, "p.patch");
    patch_ops::export(&dir, &[commit(&st, "Third: more (lines)"), commit(&st, "Fourth")], &p).unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~2"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nmine\n").unwrap();
    let mine = commit_on(&dir, "Mine", 10);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let how = ApplyHow::Commits(AmOptions::default());
    drive(&mut st, Msg::Patch(PatchMsg::Apply(ApplyRequest { workdir: dir.clone(), path: p.clone(), how, commits: 2 })));
    assert_eq!(repo_state(&st), RepositoryState::ApplyMailbox);
    let at = |st: &AppState| st.snap.as_ref().unwrap().am.as_ref().map(|a| (a.step, a.total));
    assert_eq!(at(&st), Some((2, 2)));
    resolve(&mut st, &dir, "a.txt", "one\ntwo\nmine\nfour\n");
    let stop = head_of(&dir);

    ask(&mut st, "Abort");
    let Some(Modal::ConfirmDanger { title, body, .. }) = &st.modal else { panic!("asks first") };
    assert_eq!(title, "Stop applying patches?");
    assert!(body.contains("Glowfly keeps them first"), "{body}");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir), (Some("main".into()), mine));
    assert_eq!(last_toast(&st), (true, "Stopped applying patches. The branch is back where it was.".to_owned()));
    assert_eq!(undo_top(&st).1, "Undo abort of patch series");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::ApplyMailbox);
    assert_eq!(at(&st), Some((2, 2)));
    assert_eq!(head_of(&dir), stop, "the branch back on p1");
    assert_eq!(staged_paths(&st), ["a.txt"]);
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    drive(&mut st, Msg::git("Continue", AM_CONTINUE));
    assert_eq!(last_toast(&st), (true, "Applied the remaining patches.".to_owned()));
    assert_eq!(subjects(&dir, "main")[..3], ["Fourth", "Third: more (lines)", "Mine"]);
    cleanup(&dir);
}

#[test]
fn a_rebase_with_autostash_aborts_plainly() {
    let (mut st, dir, tip) = stopped_rebase("abort-steps-autostash-flow", &["--autostash"]);
    assert!(st.snap.as_ref().unwrap().rebase.as_ref().is_some_and(|p| p.autostash));
    let Msg::ConfirmDanger { body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert!(body.ends_with("This can't be undone."), "{body}");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_of(&dir), (Some("feature".into()), tip));
    assert!(!matches!(undo_top(&st).0, Some(Action::AbortKept { .. })), "a plain abort");
    assert_eq!(read(&dir, "README.md"), "readme, dirty\n", "git put its stash back");
    let _ = std::fs::remove_dir_all(&dir);
}

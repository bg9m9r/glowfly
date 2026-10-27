//! Restacking stacked branches end to end on real repositories: the run, its undo, the offer
//! after an amend, a stop on a conflict and what Continue and Abort do with the steps left,
//! and every refusal.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{CommitMsg, JobMsg, Msg, RebaseMsg, RepoMsg};
use crate::state::rebase::STASH_LINE;
use crate::state::{AppState, Modal};
use git2::{Oid, RepositoryState};
use glowfly_core::git_version::GitInfo;
use glowfly_core::stack::RestackScope;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn commit(dir: &Path, file: &str, text: &str, msg: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

fn tip(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

fn head_name(dir: &Path) -> String {
    git2::Repository::open(dir).unwrap().head().unwrap().shorthand().unwrap().to_owned()
}

/// main <- low <- mid <- top (mid edits g.txt's first line), with low checked out; `amend`
/// rewrites low first, so mid and top need a restack.
fn stacked(name: &str, amend: bool) -> PathBuf {
    let dir = scratch_repo(name);
    // Line endings off before a.txt is checked out again, or it comes back with a CRLF the index
    // doesn't have, and git's rebase sees an unstaged change once it re-reads the file.
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    commit(&dir, "g.txt", "1\n2\n3\n", "base");
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l.txt", "l\n", "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "g.txt", "one\n2\n3\n", "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    commit(&dir, "t.txt", "t\n", "t1");
    git(&dir, &["checkout", "-q", "low"]);
    if amend {
        git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    }
    dir
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn restack_msg(scope: RestackScope) -> Msg {
    Msg::Rebase(RebaseMsg::Restack { scope, autostash: false })
}

fn above_low() -> Msg {
    restack_msg(RestackScope::Above("low".into()))
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn undo_label(st: &AppState) -> String {
    st.undo.undo_entry(st.settings.lang(), true).0
}

#[test]
fn a_restack_runs_and_undoes_as_one_entry() {
    let dir = stacked("stack-run", true);
    let (mid, top) = (tip(&dir, "mid"), tip(&dir, "top"));
    let mut st = opened(&dir);
    assert!(st.snap.as_ref().unwrap().stacks.below("mid").is_some_and(|l| l.behind));
    drive(&mut st, above_low());
    assert_eq!(last_toast(&st), (true, "Restack succeeded".into()));
    let link = st.snap.as_ref().unwrap().stacks.below("mid").cloned().expect("mid is still stacked on low");
    assert!(!link.behind && link.on == "low", "{link:?}");
    assert_eq!(tip(&dir, "mid^"), tip(&dir, "low"));
    assert_eq!(head_name(&dir), "low");
    assert!(undo_label(&st).contains("restack of the branches above low"), "{}", undo_label(&st));
    assert_eq!(st.undo.undo.len(), 1);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!((tip(&dir, "mid"), tip(&dir, "top")), (mid, top), "every moved branch is back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn amending_a_lower_branch_offers_a_restack() {
    let dir = stacked("stack-offer", false);
    let mut st = opened(&dir);
    st.session.draft.amend = true;
    st.session.draft.summary = "l1, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    let offer = st.toasts.iter().rev().find_map(|t| t.action.as_ref()).expect("a toast with an action");
    assert!(matches!(&*offer.msg, Msg::Rebase(RebaseMsg::Restack { scope: RestackScope::Above(b), .. }) if b == "low"));
    assert_eq!(offer.label, "Restack");
    let text = &st.toasts.iter().rev().find(|t| t.action.is_some()).unwrap().text;
    assert_eq!(text, "Branches are stacked on low. Restack them onto its new tip?");

    // Nothing is stacked on top: no offer.
    git(&dir, &["checkout", "-q", "top"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.toasts.clear();
    st.session.draft.amend = true;
    st.session.draft.summary = "t1, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert!(st.toasts.iter().all(|t| t.action.is_none()), "no offer for a branch with nothing above it");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `stacked` with a second leaf (top2 on mid) and low amended to conflict with mid.
fn conflicting(name: &str) -> PathBuf {
    let dir = stacked(name, false);
    git(&dir, &["checkout", "-q", "-b", "top2", "mid"]);
    commit(&dir, "t2.txt", "t2\n", "t2");
    git(&dir, &["checkout", "-q", "low"]);
    std::fs::write(dir.join("g.txt"), "uno\n2\n3\n").unwrap();
    git(&dir, &["commit", "-q", "-a", "--amend", "-m", "l1, amended"]);
    dir
}

#[test]
fn a_conflict_stops_and_continue_finishes_the_rest() {
    let dir = conflicting("stack-conflict");
    let mid = tip(&dir, "mid");
    let mut st = opened(&dir);
    drive(&mut st, above_low());
    // git's merge backend keeps its state in rebase-merge/, which libgit2 may read either way.
    let state = st.snap.as_ref().unwrap().state;
    assert!(matches!(state, RepositoryState::RebaseMerge | RepositoryState::RebaseInteractive), "{state:?}");
    assert_eq!(st.restacks.len(), 1, "the second leaf waits");
    let short = &mid.to_string()[..7];
    let want = format!("Restack stopped on a conflict at {short} (step 1 of 2). Resolve it, then Continue, or Abort.");
    assert_eq!(last_toast(&st).1, want);

    std::fs::write(dir.join("g.txt"), "both\n2\n3\n").unwrap();
    git(&dir, &["add", "g.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Rebase(RebaseMsg::Resume(glowfly_core::rebase::Resume::Continue)));
    assert_eq!(last_toast(&st), (true, "Restack succeeded".into()));
    assert!(st.restacks.is_empty());
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert_eq!(tip(&dir, "top2^"), tip(&dir, "mid"));
    assert_eq!(tip(&dir, "mid^"), tip(&dir, "low"));
    assert_eq!(head_name(&dir), "low");
    assert_eq!(st.undo.undo.len(), 1, "one entry for the whole restack");
    assert!(undo_label(&st).contains("restack of the branches above low"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_drops_the_steps_left() {
    let dir = conflicting("stack-abort");
    let mut st = opened(&dir);
    drive(&mut st, above_low());
    assert_eq!(st.restacks.len(), 1);
    drive(&mut st, Msg::git("Abort rebase", &["rebase", "--abort"]));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert!(st.restacks.is_empty());
    assert!(logged(&st, "The restack ended early. Not restacked: top, top2."), "the aborted step's top too");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_single_step_restack_dropped_after_a_stop_says_so_unless_it_finished() {
    // low <- mid <- top, low amended to conflict with mid: one step, which stops.
    let dir = stacked("stack-abort-one", false);
    std::fs::write(
        dir.join("g.txt"),
        "uno
2
3
",
    )
    .unwrap();
    git(&dir, &["commit", "-q", "-a", "--amend", "-m", "l1, amended"]);
    let mut st = opened(&dir);
    drive(&mut st, above_low());
    assert_eq!(st.restacks.len(), 1, "the stopped step waits for the banner");
    drive(&mut st, Msg::git("Abort rebase", &["rebase", "--abort"]));
    assert!(st.restacks.is_empty());
    assert!(logged(&st, "The restack ended early. Not restacked: top."));

    // Finished outside Glowfly: nothing was left undone, so nothing is logged.
    st.log.clear();
    drive(&mut st, above_low());
    assert_eq!(st.restacks.len(), 1);
    std::fs::write(
        dir.join("g.txt"),
        "both
2
3
",
    )
    .unwrap();
    git(&dir, &["add", "g.txt"]);
    git(&dir, &["-c", "core.editor=true", "rebase", "--continue"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.restacks.is_empty());
    assert!(!logged(&st, "The restack ended early"), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert_eq!(tip(&dir, "top^"), tip(&dir, "mid"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `msg` is refused with `why`: a red toast and no job.
#[track_caller]
fn refused(st: &mut AppState, msg: Msg, why: &str) {
    let fx = update(st, msg, Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    assert_eq!(last_toast(st), (false, why.to_owned()));
}

#[test]
fn refusals_start_nothing() {
    let dir = stacked("stack-refusals", false);
    let mut st = opened(&dir);
    refused(&mut st, above_low(), "The branches above low are already on its tip.");

    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 37, 1)), chosen: false });
    refused(&mut st, above_low(), "Restacking branches needs git 2.38 or newer (you have 2.37.1).");
    st.git.info = None;

    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    let busy = crate::state::rebase_onto::in_progress(st.settings.lang(), RepositoryState::Merge).unwrap();
    refused(&mut st, above_low(), busy);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    let other = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&other);
    git(&dir, &["worktree", "add", "-q", &other.display().to_string(), "mid"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    refused(&mut st, above_low(), "mid is checked out in another worktree; restack it from there.");
    git(&dir, &["worktree", "remove", "--force", &other.display().to_string()]);

    st.job = Some(super::rebase_tests::running_job(&dir));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, above_low(), Instant::now());
    assert!(fx.is_empty());
    assert!(logged(&st, "Another operation is still running."));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn a_dirty_tree_asks_then_stashes() {
    let dir = stacked("stack-stash", true);
    let mut st = opened(&dir);
    std::fs::write(dir.join("l.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(update(&mut st, above_low(), Instant::now()).is_empty());
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. }) => {
            assert_eq!((title.as_str(), body.as_str(), button.as_str()), ("Stash changes and restack", STASH_LINE, "Restack"));
        }
        _ => panic!("expected the stash question"),
    }
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::Restack { job, .. })] if job.plan.autostash), "{fx:?}");
    finish(&mut st, fx);
    assert_eq!(last_toast(&st), (true, "Restack succeeded".into()));
    assert_eq!(std::fs::read_to_string(dir.join("l.txt")).unwrap(), "edited\n");
    assert_eq!(head_name(&dir), "low");
    assert_eq!(tip(&dir, "mid^"), tip(&dir, "low"));
    let _ = std::fs::remove_dir_all(&dir);
}

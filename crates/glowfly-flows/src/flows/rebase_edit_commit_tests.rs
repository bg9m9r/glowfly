//! Headless tests for the commit form at a rebase stopped to edit a commit: Amend ticked once per
//! stop and the draft filled from the commit, Amend and continue amending and finishing the
//! rebase in one go, Commit and continue with Amend unticked, an amend that would change nothing
//! refused, Amend unticked again once the rebase is over, never amending the commit before an
//! `edit` step that conflicted or after a reset at the stop, and a failed commit that leaves the
//! rebase stopped.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{CommitMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::commit::commit_blocked;
use crate::state::commit::stop::{amend_blocked_in, amends_at_stop, edit_stop};
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run, run_with_input};
use glowfly_core::i18n::Lang;
use std::path::{Path, PathBuf};

/// A repository with "first" and "second", stopped by an interactive rebase to edit "second".
pub(super) fn stopped_at_edit(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-q", "-am", "second"]);
    // git runs the sequence editor through sh with the todo file as its argument.
    let editor = "f() { sed 's/^pick/edit/' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; }; f";
    let (ok, out) = run_with_input(&dir, &args(&["rebase", "-i", "HEAD~1"]), "", &[("GIT_SEQUENCE_EDITOR", editor)]);
    assert!(ok, "{out}");
    dir
}

fn open(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn log(dir: &Path) -> Vec<String> {
    let (ok, out) = run(dir, &args(&["log", "--format=%s"]));
    assert!(ok, "{out}");
    out.lines().filter(|l| !l.starts_with("$ ") && !l.trim().is_empty()).map(|l| l.trim().to_owned()).collect()
}

fn state_of(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

#[test]
fn amend_and_continue_amends_the_commit_and_finishes_the_rebase() {
    let dir = stopped_at_edit("edit-stop-amend");
    let mut st = open(&dir);
    assert!(edit_stop(st.snap.as_ref().unwrap()).is_some(), "stopped to edit");
    let d = &st.session.draft;
    assert!(d.amend, "Amend is ticked at a stop to edit");
    assert_eq!(d.summary, "second", "the draft holds the commit's message");
    assert!(commit_blocked(&st, false).is_some(), "nothing staged and the message unchanged: nothing to amend");

    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.session.draft.amend, "still ticked: the same stop");
    st.session.draft.summary = "second, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));

    assert_eq!(state_of(&st), RepositoryState::Clean, "the rebase went on and finished");
    assert_eq!(log(&dir), ["second, amended", "first"], "amended, not a new commit");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace('\r', ""), "one\ntwo\nthree\n");
    assert!(!st.session.draft.amend && st.session.draft.amend_stop.is_none(), "Amend is off again");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn commit_and_continue_adds_a_commit_when_amend_is_unticked() {
    let dir = stopped_at_edit("edit-stop-commit");
    let mut st = open(&dir);
    st.session.draft.amend = false;
    st.session.draft.summary = "inserted".into();
    st.session.draft.description.clear();
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!st.session.draft.amend, "unticked stays unticked at the same stop");
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    assert_eq!(log(&dir), ["inserted", "second", "first"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn commit_and_continue_does_nothing_outside_a_stop_to_edit() {
    let dir = scratch_repo("edit-stop-none");
    git(&dir, &["add", "a.txt"]);
    let mut st = open(&dir);
    assert!(!st.session.draft.amend, "no stop, no Amend");
    st.session.draft.summary = "plain".into();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(log(&dir), ["first"], "nothing committed");
    let _ = std::fs::remove_dir_all(dir);
}

fn rev(dir: &Path, r: &str) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", r]));
    assert!(ok, "{out}");
    out.lines().rfind(|l| !l.starts_with("$ ")).unwrap_or("").trim().to_owned()
}

#[test]
fn an_edit_step_that_conflicted_never_amends_the_commit_before_it() {
    let dir = scratch_repo("edit-stop-conflict");
    git(&dir, &["commit", "-q", "-am", "second"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "third"]);
    let first = rev(&dir, "HEAD~2");
    // Drop "second" and edit "third", which then doesn't apply cleanly on "first".
    let editor = "f() { sed -e '1d' -e 's/^pick/edit/' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; }; f";
    let (ok, out) = run_with_input(&dir, &args(&["rebase", "-i", "HEAD~2"]), "", &[("GIT_SEQUENCE_EDITOR", editor)]);
    assert!(!ok, "the edit step conflicts: {out}");
    let mut st = open(&dir);
    let snap = st.snap.as_ref().unwrap();
    assert!(edit_stop(snap).is_some(), "git reports the edit step");
    assert!(!amends_at_stop(snap), "but HEAD is still the commit before it");
    assert!(!st.session.draft.amend, "Amend isn't ticked");
    assert!(amend_blocked_in(Lang::En, snap).is_some(), "nor can it be");

    std::fs::write(dir.join("a.txt"), "one\nthree\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!st.session.draft.amend);
    st.session.draft.amend = true;
    assert!(commit_blocked(&st, false).is_some(), "an amend here is refused");
    st.session.draft.amend = false;
    st.session.draft.summary = "third".into();
    st.session.draft.description.clear();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    assert_eq!(log(&dir), ["third", "first"]);
    assert_eq!(rev(&dir, "HEAD~1"), first, "the commit before is untouched");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_reset_at_a_stop_to_edit_unticks_amend() {
    let dir = stopped_at_edit("edit-stop-reset");
    let first = rev(&dir, "HEAD~1");
    let mut st = open(&dir);
    assert!(st.session.draft.amend, "ticked at the clean stop");
    // Split the commit: HEAD goes back to "first", its changes stay in the working tree.
    git(&dir, &["reset", "-q", "HEAD~1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!amends_at_stop(st.snap.as_ref().unwrap()));
    assert!(!st.session.draft.amend, "unticked: amending would change the commit before");
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.session.draft.summary = "part one".into();
    st.session.draft.description.clear();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    assert_eq!(log(&dir), ["part one", "first"]);
    assert_eq!(rev(&dir, "HEAD~1"), first, "the commit before is untouched");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failed_commit_leaves_the_rebase_stopped_and_keeps_the_draft() {
    let dir = stopped_at_edit("edit-stop-hook");
    let hooks = dir.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho refused >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(hooks.join("pre-commit"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let head = rev(&dir, "HEAD");
    let mut st = open(&dir);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.session.draft.summary = "second, amended".into();
    let stop = edit_stop(st.snap.as_ref().unwrap());
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(state_of(&st), RepositoryState::RebaseInteractive, "the rebase didn't go on");
    assert_eq!(edit_stop(st.snap.as_ref().unwrap()), stop, "still at the same stop");
    assert_eq!(rev(&dir, "HEAD"), head, "nothing was amended");
    assert_eq!(st.session.draft.summary, "second, amended", "the draft is kept");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stage_all_and_commit_at_a_stop_to_edit_continues_too() {
    let dir = stopped_at_edit("edit-stop-stage-all");
    let mut st = open(&dir);
    assert!(commit_blocked(&st, true).is_some(), "nothing to stage and the message unchanged: nothing to amend");
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit));
    assert_eq!(state_of(&st), RepositoryState::Clean, "it went on, as the button does");
    assert_eq!(log(&dir), ["second", "first"], "amended");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace('\r', ""), "one\ntwo\nthree\n");
    let _ = std::fs::remove_dir_all(dir);
}

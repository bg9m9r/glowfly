//! Headless tests for undoing a merge, cherry-pick or revert that stopped for conflicts and was
//! finished later: from the commit box, or the in-progress banner's Continue. The finished
//! operation is one undo step back to the HEAD it started at, and Skip keeps the history.

use super::cherry_pick_tests::feature_repo;
use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{CommitMsg, JobMsg, Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::AppState;
use crate::state::rebase_banner::action_msg;
use git2::{Oid, RepositoryState};
use glowfly_core::pick_ops::CHERRY_PICK_SKIP;
use glowfly_core::undo::{Action, MoveOp};
use std::path::{Path, PathBuf};

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().expect("a repository is open");
    snap.commits[snap.row_of[&snap.head_oid.expect("HEAD")]].summary.to_string()
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

fn tooltip(st: &AppState) -> String {
    st.session.undo.undo_entry(st.settings.lang(), true).0
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap().replace("\r\n", "\n")
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Writes `text` to `file` and stages it, as resolving a conflict in the app would.
fn resolve(st: &mut AppState, dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    drive(st, Msg::Stage(StageMsg::StageFile(file.into())));
}

fn commit(st: &mut AppState, summary: &str) {
    st.session.draft.summary = summary.into();
    drive(st, Msg::Commit(CommitMsg::Commit));
}

/// main ("ours") and other ("theirs") edit a.txt's second line; opened, with b.txt committed
/// through the app ("add b") so the history holds one entry.
fn merge_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("a.txt"), "one\ntheirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nours\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("b.txt".into())));
    commit(&mut st, "add b");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Commit { .. })), "{:?}", st.session.undo.undo);
    (st, dir)
}

#[test]
fn merge_finished_from_the_commit_box() {
    let (mut st, dir) = merge_repo("undo-cont-merge");
    drive(&mut st, Msg::git("Merge", &["merge", "other"]));
    assert_eq!(repo_state(&st), RepositoryState::Merge);
    assert_eq!(st.session.undo.undo.len(), 1, "the failed merge keeps the history");
    assert!(st.session.undo.blocked.is_none());

    resolve(&mut st, &dir, "a.txt", "one\nmerged\n");
    commit(&mut st, "Merge other");
    assert!(
        matches!(st.session.undo.undo.last(), Some(Action::Moved { op: MoveOp::Merge, detail: Some(d), .. }) if d == "other"),
        "{:?}",
        st.session.undo.undo
    );
    assert!(tooltip(&st).starts_with("Undo merge of other"), "{}", tooltip(&st));
    // add b, marking a.txt resolved (refused as over once the merge is undone), the merge.
    assert_eq!(st.session.undo.undo.len(), 3);
    let merged = head_summary(&st);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "add b");
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(read(&dir, "a.txt"), "one\nours\n");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(head_summary(&st), merged);
    assert_eq!(read(&dir, "a.txt"), "one\nmerged\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_pick_continued_from_the_banner() {
    let (mut st, dir, picks) = feature_repo("undo-cont-pick", &[("f1", "base\n1\n")], Some("base\nmain\n"));
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![picks[0]])));
    assert_eq!(repo_state(&st), RepositoryState::CherryPick);
    resolve(&mut st, &dir, "f.txt", "base\nmain\n1\n");
    drive(&mut st, action_msg(RepositoryState::CherryPick, "Continue", &["cherry-pick", "--continue"]));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { op: MoveOp::CherryPick, .. })), "{:?}", st.session.undo.undo);
    assert!(tooltip(&st).starts_with(&format!("Undo cherry-pick of {}", short(picks[0]))), "{}", tooltip(&st));
    assert!(st.session.undo.blocked.is_none());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "main edit");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(head_summary(&st), "f1");
    assert!(st.session.undo.blocked.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multi_pick_continued_is_one_step() {
    let feature = [("f1", "base\n1\n"), ("f2", "base\n1\n2\n")];
    let (mut st, dir, picks) = feature_repo("undo-cont-multi", &feature, Some("base\nmain\n"));
    let before = st.session.undo.undo.len();
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(picks.clone())));
    assert_eq!(repo_state(&st), RepositoryState::CherryPickSequence);
    resolve(&mut st, &dir, "f.txt", "base\n1\n");
    drive(&mut st, action_msg(RepositoryState::CherryPickSequence, "Continue", &["cherry-pick", "--continue"]));
    assert_eq!(head_summary(&st), "f2");
    // Marking f.txt resolved, then the whole pick as one step.
    assert_eq!(st.session.undo.undo.len(), before + 2, "one step: {:?}", st.session.undo.undo);
    assert!(matches!(st.session.undo.undo[before], Action::Conflicts { .. }), "{:?}", st.session.undo.undo);
    assert_eq!(st.session.undo.undo.last().unwrap().describe(), "cherry-pick of 2 commits");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "main edit");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_continued_from_the_banner() {
    let dir = scratch_repo("undo-cont-revert");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("a.txt"), "one\nA\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "A"]);
    let a = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    std::fs::write(dir.join("a.txt"), "one\nB\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "B"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::git("Revert", &["revert", &a.to_string()]));
    assert_eq!(repo_state(&st), RepositoryState::Revert);
    resolve(&mut st, &dir, "a.txt", "one\n");
    drive(&mut st, action_msg(RepositoryState::Revert, "Continue", &["revert", "--continue"]));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { op: MoveOp::Revert, .. })), "{:?}", st.session.undo.undo);
    assert!(tooltip(&st).starts_with(&format!("Undo revert of {}", short(a))), "{}", tooltip(&st));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "B");
    assert_eq!(read(&dir, "a.txt"), "one\nB\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_keeps_the_history() {
    let (mut st, dir, picks) = feature_repo("undo-cont-skip", &[("f1", "base\n1\n")], Some("base\nmain\n"));
    std::fs::write(dir.join("g.txt"), "g\n").unwrap();
    drive(&mut st, Msg::Stage(StageMsg::StageFile("g.txt".into())));
    commit(&mut st, "add g");
    let history = st.session.undo.undo.clone();
    assert!(matches!(history.last(), Some(Action::Commit { .. })), "{history:?}");

    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![picks[0]])));
    assert_eq!(repo_state(&st), RepositoryState::CherryPick);
    drive(&mut st, action_msg(RepositoryState::CherryPick, "Skip commit", CHERRY_PICK_SKIP));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_summary(&st), "add g");
    assert_eq!(st.session.undo.undo, history, "nothing added, nothing lost");
    assert!(st.session.undo.blocked.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

//! The operation in progress: the strip's Show work in progress, and the banner's Abort, Skip
//! and Reset bisect asking first, with a merge or single cherry-pick aborted into a stash that
//! Undo pops back.

use super::cherry_pick_tests::feature_repo;
use super::conflict_tests::{conflicted_repo, two_conflicts};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::op_actions::state_actions;
use crate::state::op_confirm::banner_msg;
use crate::state::{AppState, Modal, Selection};
use git2::{Repository, RepositoryState};
use glowfly_core::undo::{ABORT_STASH_MESSAGE, Action};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The banner message of the action of the current state whose label starts with `prefix`.
fn banner(st: &AppState, prefix: &str) -> Msg {
    let state = st.snap.as_ref().unwrap().state;
    let (label, args) = *state_actions(state).unwrap().iter().find(|(l, _)| l.starts_with(prefix)).unwrap();
    banner_msg(st, label, args)
}

/// Presses that banner button (which opens its confirmation).
fn ask(st: &mut AppState, prefix: &str) {
    let m = banner(st, prefix);
    drive(st, m);
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap().replace("\r\n", "\n")
}

/// The message of the newest stash, if any.
fn top_stash(dir: &Path) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let id = repo.refname_to_id("refs/stash").ok()?;
    repo.find_commit(id).unwrap().message().map(str::to_owned)
}

/// Writes `text` to `file`, stages it and refreshes.
fn resolve(st: &mut AppState, dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", file]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

#[test]
fn abort_merge_asks_naming_resolved_files() {
    let dir = two_conflicts("op-abort-asks");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nboth\n");
    let Msg::ConfirmDanger { title, body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert_eq!(title, "Abort the merge?");
    for line in
        ["The 1 file you resolved goes back to how it was.", "Edits in the 1 conflicted file are lost too.", "This can't be undone."]
    {
        assert!(body.contains(line), "{body}");
    }
    ask(&mut st, "Abort");
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })));
    assert_eq!(repo_state(&st), RepositoryState::Merge, "nothing ran yet");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_merge_all_resolved_stashes_and_undo_restores() {
    let dir = conflicted_repo("op-abort-keep");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nmerged\n");
    let Msg::ConfirmDanger { body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert!(body.ends_with("They are kept in a stash first, so Undo brings them back."), "{body}");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(read(&dir, "a.txt"), "one\nours\n");
    assert!(top_stash(&dir).is_some_and(|m| m.contains(ABORT_STASH_MESSAGE)));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashPush { .. })), "recorded for undo");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt"), "one\nmerged\n");
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "a.txt"), "back staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_leaves_an_unrelated_edit_in_place() {
    let dir = scratch_repo("op-abort-unrelated");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(
        dir.join("README.md"),
        "readme
",
    )
    .unwrap();
    git(&dir, &["commit", "-q", "-am", "base"]);
    git(&dir, &["add", "README.md"]);
    git(&dir, &["commit", "-q", "-m", "readme"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(
        dir.join("a.txt"),
        "one
theirs
",
    )
    .unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(
        dir.join("a.txt"),
        "one
ours
",
    )
    .unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    std::fs::write(
        dir.join("README.md"),
        "readme
mine
",
    )
    .unwrap();
    let (ok, _) = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflict");
    let mut st = opened(&dir);
    resolve(
        &mut st,
        &dir,
        "a.txt",
        "one
merged
",
    );
    let Msg::ConfirmDanger { body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert!(body.ends_with("This can't be undone."), "no stash promised: {body}");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(
        read(&dir, "a.txt"),
        "one
ours
"
    );
    assert_eq!(
        read(&dir, "README.md"),
        "readme
mine
",
        "the unrelated edit stays in the working tree"
    );
    assert_eq!(top_stash(&dir), None, "nothing hidden in a stash");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_with_conflicts_left_is_plain() {
    let dir = two_conflicts("op-abort-plain");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nboth\n");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(top_stash(&dir), None, "no stash made");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_keep_refused_when_conflicts_return() {
    let dir = conflicted_repo("op-abort-refused");
    let mut st = opened(&dir);
    let abort = JobMsg::Abort { label: "Abort merge".into(), args: glowfly_core::gitcli::args(&["merge", "--abort"]), keep: true };
    let fx = update(&mut st, Msg::Job(abort), Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "nothing runs");
    assert!(st.toasts.iter().any(|t| !t.ok && t.text.contains("Try again")));
    assert_eq!(repo_state(&st), RepositoryState::Merge);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The feature fixture stopped cherry-picking its first commit (or both, with `two`).
fn stopped_pick(name: &str, two: bool) -> (AppState, PathBuf) {
    let picks: &[(&str, &str)] = if two { &[("x1", "theirs\n"), ("x2", "theirs\nmore\n")] } else { &[("x1", "theirs\n")] };
    let (mut st, dir, f) = feature_repo(name, picks, Some("ours\n"));
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(f)));
    let s = repo_state(&st);
    assert!(matches!(s, RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{s:?}");
    (st, dir)
}

#[test]
fn single_cherry_pick_abort_keeps() {
    let (mut st, dir) = stopped_pick("op-pick-keep", false);
    assert_eq!(repo_state(&st), RepositoryState::CherryPick);
    resolve(&mut st, &dir, "f.txt", "merged\n");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(read(&dir, "f.txt"), "ours\n");
    assert!(top_stash(&dir).is_some_and(|m| m.contains(ABORT_STASH_MESSAGE)));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashPush { .. })));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "f.txt"), "merged\n");
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "f.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_and_reset_bisect_ask_first() {
    let (mut st, dir) = stopped_pick("op-skip-asks", true);
    let before = repo_state(&st);
    ask(&mut st, "Skip");
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { ref title, .. }) if title == "Skip this commit?"));
    drive(&mut st, Msg::CloseModal);
    assert_eq!(repo_state(&st), before, "Cancel runs nothing");
    assert!(st.modal.is_none());
    ask(&mut st, "Skip");
    drive(&mut st, Msg::ConfirmAccepted);
    let after = repo_state(&st);
    assert!(matches!(after, RepositoryState::Clean | RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{after:?}");
    assert!(!git_out(&dir, &["log", "--format=%s"]).lines().any(|l| l == "x1"), "x1 left out");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = scratch_repo("op-bisect");
    git(&dir, &["commit", "-q", "-am", "second"]);
    let mut st = opened(&dir);
    git(&dir, &["bisect", "start"]);
    git(&dir, &["bisect", "bad"]);
    git(&dir, &["bisect", "good", "HEAD~1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(repo_state(&st), RepositoryState::Bisect);
    ask(&mut st, "Reset");
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { ref title, .. }) if title == "End the bisect?"));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}

fn git_out(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = glowfly_core::gitcli::run(dir, &glowfly_core::gitcli::args(a));
    assert!(ok, "{out}");
    out.trim().to_owned()
}

#[test]
fn show_in_progress() {
    let dir = conflicted_repo("op-show");
    let mut st = opened(&dir);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)));
    st.settings.show_details_panel = false;
    st.view.scroll_to_row = None;
    let fx = update(&mut st, Msg::Repo(RepoMsg::ShowInProgress), Instant::now());
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.settings.show_details_panel);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "saved like the other panel toggles");
    assert!(st.snap.as_ref().unwrap().graph.wip.is_some());
    assert_eq!(st.view.scroll_to_row, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

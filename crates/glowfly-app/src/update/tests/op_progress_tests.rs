//! The operation in progress: the strip's Show work in progress, and the banner's Abort, Skip
//! and Reset bisect asking first, with a merge or single cherry-pick aborted keeping its work
//! (also with conflicts left or unrelated edits), which Undo puts back in progress.

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
use glowfly_core::i18n::Lang;
use glowfly_core::undo::Action;
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

/// The git folder's `name` file, if any.
fn git_file(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(".git").join(name)).ok()
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

/// The undo history's top, and Edit > Undo's label for it.
fn undo_top(st: &AppState) -> (Option<&Action>, String) {
    (st.session.undo.undo.last(), st.session.undo.undo_entry(Lang::En, true).0)
}

fn conflicted(st: &AppState) -> Vec<&str> {
    st.snap.as_ref().unwrap().unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect()
}

fn staged_paths(st: &AppState) -> Vec<&str> {
    st.snap.as_ref().unwrap().staged.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn abort_merge_asks_naming_resolved_and_conflicted_files() {
    let dir = two_conflicts("op-abort-asks");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nboth\n");
    let Msg::ConfirmDanger { title, body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert_eq!(title, "Abort the merge?");
    for line in [
        "The 1 file you resolved goes back to how it was.",
        "Edits in the 1 conflicted file go too.",
        "Glowfly keeps them first, so Undo brings them back and picks up where you left off.",
    ] {
        assert!(body.contains(line), "{body}");
    }
    ask(&mut st, "Abort");
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })));
    assert_eq!(repo_state(&st), RepositoryState::Merge, "nothing ran yet");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_merge_all_resolved_keeps_and_undo_restores() {
    let dir = conflicted_repo("op-abort-keep");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nmerged\n");
    let merge_head = git_file(&dir, "MERGE_HEAD");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(read(&dir, "a.txt"), "one\nours\n");
    assert_eq!(top_stash(&dir), None, "kept outside the stash list");
    let (top, label) = undo_top(&st);
    assert!(matches!(top, Some(Action::AbortKept { .. })), "recorded for undo");
    assert_eq!(label, "Undo abort of merge");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::Merge, "the merge is in progress again");
    assert_eq!(git_file(&dir, "MERGE_HEAD"), merge_head);
    assert_eq!(read(&dir, "a.txt"), "one\nmerged\n");
    assert_eq!(staged_paths(&st), ["a.txt"], "back staged");
    assert_eq!(st.session.undo.redo_entry(Lang::En, true).0, "Redo abort of merge");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_with_conflicts_left_is_undone_back_in_progress() {
    let dir = two_conflicts("op-abort-partial");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nboth\n");
    let edited = read(&dir, "b.txt").replace("ours", "ours, edited");
    std::fs::write(dir.join("b.txt"), &edited).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(conflicted(&st), ["b.txt"]);
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!((read(&dir, "a.txt"), read(&dir, "b.txt")), ("one\nours\n".into(), "one\nours\n".into()));
    assert_eq!(top_stash(&dir), None);
    assert_eq!(undo_top(&st).1, "Undo abort of merge");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::Merge);
    assert_eq!(read(&dir, "a.txt"), "one\nboth\n", "the resolution is intact");
    assert_eq!(staged_paths(&st), ["a.txt"]);
    assert_eq!(read(&dir, "b.txt"), edited, "the conflicted file keeps its edits");
    assert_eq!(conflicted(&st), ["b.txt"], "and is still conflicted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_with_an_unrelated_edit_keeps_both_and_undo_restores() {
    let dir = scratch_repo("op-abort-unrelated");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("README.md"), "readme\n").unwrap();
    git(&dir, &["add", "README.md"]);
    git(&dir, &["commit", "-q", "-m", "readme"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("a.txt"), "one\ntheirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nours\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    std::fs::write(dir.join("README.md"), "readme\nmine\n").unwrap();
    let (ok, _) = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflict");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nmerged\n");
    let Msg::ConfirmDanger { body, .. } = banner(&st, "Abort") else { panic!("asks first") };
    assert!(body.ends_with("Your other changes stay as they are."), "{body}");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(read(&dir, "a.txt"), "one\nours\n");
    assert_eq!(read(&dir, "README.md"), "readme\nmine\n", "the unrelated edit stays in the working tree");
    assert_eq!(top_stash(&dir), None, "nothing hidden in a stash");
    assert_eq!(undo_top(&st).1, "Undo abort of merge");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::Merge);
    assert_eq!(read(&dir, "a.txt"), "one\nmerged\n", "the resolution is intact");
    assert_eq!(staged_paths(&st), ["a.txt"]);
    assert_eq!(read(&dir, "README.md"), "readme\nmine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_of_a_kept_abort_refuses_when_its_file_changed() {
    let dir = conflicted_repo("op-abort-undo-dirty");
    let mut st = opened(&dir);
    resolve(&mut st, &dir, "a.txt", "one\nmerged\n");
    ask(&mut st, "Abort");
    drive(&mut st, Msg::ConfirmAccepted);
    std::fs::write(dir.join("a.txt"), "one\nnew work\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::Clean, "nothing put back");
    assert_eq!(read(&dir, "a.txt"), "one\nnew work\n", "the new edit is untouched");
    assert!(st.toasts.iter().any(|t| !t.ok && t.text.contains("abort of merge")), "says why");
    assert!(matches!(undo_top(&st).0, Some(Action::AbortKept { .. })), "the entry stays for later");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abort_keep_refused_when_the_operation_ended_meanwhile() {
    let dir = conflicted_repo("op-abort-refused");
    let mut st = opened(&dir);
    git(&dir, &["merge", "--abort"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let abort = JobMsg::Abort { label: "Abort merge".into(), args: glowfly_core::gitcli::args(&["merge", "--abort"]), keep: true };
    let fx = update(&mut st, Msg::Job(abort), Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "nothing runs");
    assert!(st.toasts.iter().any(|t| !t.ok && t.text.contains("Try again")));
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
    assert_eq!(undo_top(&st).1, "Undo abort of cherry-pick");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(repo_state(&st), RepositoryState::CherryPick, "the pick is in progress again");
    assert!(git_file(&dir, "CHERRY_PICK_HEAD").is_some());
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

//! Headless tests, end to end against real git, for cherry-picks and reverts whose commits change
//! nothing: they are skipped and said so in a toast, a conflict after them still stops, and a git
//! without `--skip` ends a single pick cleanly.

use super::selection_tests::{Fixture, commit_on, fixture, git, head, refresh};
use super::tests::drive;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::journal::short;
use crate::state::{AppState, Selection};
use git2::{Oid, RepositoryState};
use glowfly_core::git_version::GitInfo;

fn toasts(st: &AppState) -> Vec<(bool, &str)> {
    st.toasts.iter().map(|t| (t.ok, t.text.as_str())).collect()
}

fn state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

/// `topic`, off c5, with a commit per file in `files` (each adding it); back on main.
fn topic(f: &Fixture, files: &[&str]) -> Vec<Oid> {
    git(&f.dir, &["checkout", "-q", "-b", "topic", &f.c(5).to_string()]);
    let oids = files
        .iter()
        .enumerate()
        .map(|(i, name)| {
            std::fs::write(f.dir.join(name), format!("{name}\n")).unwrap();
            commit_on(&f.dir, &format!("add {name}"), 10 + i as u32)
        })
        .collect();
    git(&f.dir, &["checkout", "-q", "main"]);
    oids
}

#[test]
fn a_pick_the_branch_already_has_changes_nothing() {
    let (mut st, f) = fixture("empty-pick-app");
    let t = topic(&f, &["x.txt"]);
    git(&f.dir, &["cherry-pick", &t[0].to_string()]);
    refresh(&mut st);
    let (before, undo) = (head(&f.dir), st.undo.undo.len());
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![t[0]])));
    let text = format!("Nothing to cherry-pick: the branch already has the changes of {}.", short(t[0]));
    assert!(toasts(&st).contains(&(true, text.as_str())), "{:?}", toasts(&st));
    assert!(st.job.is_none());
    assert_eq!((state(&st), head(&f.dir)), (RepositoryState::Clean, before));
    assert_eq!(st.undo.undo.len(), undo, "nothing new to undo");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_sequence_skips_the_commit_already_applied() {
    let (mut st, f) = fixture("empty-pick-app-middle");
    let t = topic(&f, &["x.txt", "y.txt", "z.txt"]);
    git(&f.dir, &["cherry-pick", &t[1].to_string()]);
    refresh(&mut st);
    let undo = st.undo.undo.len();
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(t.clone())));
    let text = format!("Cherry-picked 2 of 3 commits; skipped {}, whose changes the branch already has.", short(t[1]));
    assert!(toasts(&st).contains(&(true, text.as_str())), "{:?}", toasts(&st));
    assert!(st.log.iter().any(|l| l.text == format!("Skipped because they change nothing on this branch: {}", short(t[1]))));
    assert_eq!(state(&st), RepositoryState::Clean);
    assert!(f.dir.join("x.txt").exists() && f.dir.join("z.txt").exists());
    assert_eq!(st.undo.undo.len(), undo + 1, "one undoable action");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn an_old_git_aborts_a_single_empty_pick() {
    let (mut st, f) = fixture("empty-pick-app-old-git");
    let t = topic(&f, &["x.txt"]);
    git(&f.dir, &["cherry-pick", &t[0].to_string()]);
    refresh(&mut st);
    st.git.info = Some(GitInfo { path: None, version: Some((2, 22, 0)), chosen: false });
    let fx = super::selection_tests::up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![t[0]])));
    assert!(matches!(&fx[..], [Effect::Pick(j)] if !j.can_skip), "{fx:?}");
    super::tests::finish(&mut st, fx);
    let text = format!("Nothing to cherry-pick: the branch already has the changes of {}.", short(t[0]));
    assert!(toasts(&st).contains(&(true, text.as_str())), "{:?}", toasts(&st));
    assert_eq!(state(&st), RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_conflict_after_a_skipped_commit_still_stops() {
    let (mut st, f) = fixture("empty-pick-app-conflict");
    let t = topic(&f, &["x.txt"]);
    git(&f.dir, &["checkout", "-q", "topic"]);
    std::fs::write(f.dir.join("f.txt"), "topic\n").unwrap();
    let clash = commit_on(&f.dir, "topic edit", 20);
    git(&f.dir, &["checkout", "-q", "main"]);
    std::fs::write(f.dir.join("f.txt"), "main\n").unwrap();
    commit_on(&f.dir, "main edit", 21);
    git(&f.dir, &["cherry-pick", &t[0].to_string()]);
    refresh(&mut st);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![t[0], clash])));
    assert!(matches!(state(&st), RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{:?}", state(&st));
    assert_eq!(st.selection, Selection::Wip, "the banner shows");
    assert!(toasts(&st).iter().any(|(ok, _)| !ok), "{:?}", toasts(&st));
    assert!(!toasts(&st).iter().any(|(_, t)| t.starts_with("Cherry-picked")), "the conflict is what the toast says");
    assert!(st.log.iter().any(|l| l.text.starts_with("Skipped") && l.text.contains(&short(t[0]))));
    git(&f.dir, &["cherry-pick", "--abort"]);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_revert_already_undone_changes_nothing() {
    let (mut st, f) = fixture("empty-revert-app");
    git(&f.dir, &["revert", "--no-edit", &f.c(5).to_string()]);
    refresh(&mut st);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = head(&f.dir);
    drive(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f.c(5)])));
    let text = format!("Nothing to revert: the changes of {} are already undone on this branch.", short(f.c(5)));
    assert!(toasts(&st).contains(&(true, text.as_str())), "{:?}", toasts(&st));
    assert_eq!((state(&st), head(&f.dir)), (RepositoryState::Clean, before));
    let _ = std::fs::remove_dir_all(&f.dir);
}

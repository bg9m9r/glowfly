//! Headless tests for the question before applying or popping a stash on top of uncommitted
//! changes: none on a clean tree, the Confirm on a dirty one (sub-list and Pop stash entries
//! alike), the job once it is accepted, and Delete's own question.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::state::palette::{PaletteCommand as C, StashAction};
use crate::state::stashes::drop_confirm_body;
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn palette(st: &mut AppState, cmd: C) -> Vec<Effect> {
    run(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)))
}

/// The commands and revision guard of the one RunGit in `fx`.
fn git_job(fx: &[Effect]) -> (Vec<Vec<String>>, Option<(String, git2::Oid)>) {
    match fx {
        [Effect::RunGit { cmds, expect, .. }] => (cmds.clone(), expect.clone()),
        _ => panic!("expected one RunGit, got {} effects", fx.len()),
    }
}

fn argv(a: &[&str]) -> Vec<Vec<String>> {
    vec![a.iter().map(|s| s.to_string()).collect()]
}

/// The open Confirm's title, body and button.
fn confirm(st: &AppState) -> (String, String, String) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. } | Modal::ConfirmDanger { title, body, button, .. }) => {
            (title.clone(), body.clone(), button.clone())
        }
        _ => panic!("no confirmation is open"),
    }
}

/// A clean repository with one stash ("On main: wip").
fn one_stash(name: &str) -> (AppState, PathBuf, git2::Oid) {
    let (mut st, dir) = repo_with_topic(name);
    git(&dir, &["stash", "push", "-m", "wip"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!(snap.stashes.len(), 1);
    let oid = snap.stashes[0].oid;
    (st, dir, oid)
}

fn make_dirty(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join("a.txt"), "one\nchanged\n").unwrap();
    drive(st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().unwrap().is_dirty());
}

#[test]
fn clean_tree_runs_straight_away() {
    let (mut st, dir, oid) = one_stash("stash-confirm-clean");
    let (cmds, expect) = git_job(&palette(&mut st, C::StashOp { action: StashAction::Apply, oid }));
    assert_eq!((cmds, expect), (argv(&["stash", "apply", "stash@{0}"]), Some(("stash@{0}".into(), oid))));
    assert!(st.modal.is_none());
    st.job = None;
    let (cmds, _) = git_job(&palette(&mut st, C::StashOp { action: StashAction::Pop, oid }));
    assert_eq!(cmds, argv(&["stash", "pop", "stash@{0}"]));
    st.job = None;
    assert_eq!(git_job(&palette(&mut st, C::PopStash)).0, argv(&["stash", "pop"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dirty_tree_asks_first() {
    let (mut st, dir, oid) = one_stash("stash-confirm-dirty");
    make_dirty(&mut st, &dir);
    let message = st.snap.as_ref().unwrap().stashes[0].message.clone();

    assert!(palette(&mut st, C::StashOp { action: StashAction::Apply, oid }).is_empty(), "nothing runs yet");
    assert!(st.palette.is_none(), "the palette closed before the question");
    let (title, body, button) = confirm(&st);
    assert_eq!((title.as_str(), button.as_str()), ("Apply stash", "Apply"));
    assert!(body.contains(&message) && body.contains("uncommitted changes"), "{body}");
    assert!(!body.contains("the stash is kept"));
    let (cmds, expect) = git_job(&run(&mut st, Msg::ConfirmAccepted));
    assert_eq!((cmds, expect), (argv(&["stash", "apply", "stash@{0}"]), Some(("stash@{0}".into(), oid))));
    st.job = None;

    assert!(palette(&mut st, C::StashOp { action: StashAction::Pop, oid }).is_empty());
    let (title, body, button) = confirm(&st);
    assert_eq!((title.as_str(), button.as_str()), ("Pop stash", "Pop"));
    assert!(body.contains(&message) && body.contains("the stash is kept"), "{body}");
    assert_eq!(git_job(&run(&mut st, Msg::ConfirmAccepted)).0, argv(&["stash", "pop", "stash@{0}"]));
    st.job = None;

    assert!(palette(&mut st, C::PopStash).is_empty(), "the Pop stash entry asks too");
    assert_eq!(confirm(&st).0, "Pop stash");
    assert_eq!(git_job(&run(&mut st, Msg::ConfirmAccepted)).0, argv(&["stash", "pop"]));
    st.job = None;

    assert!(palette(&mut st, C::StashOp { action: StashAction::Drop, oid }).is_empty());
    let (title, body, _) = confirm(&st);
    assert_eq!((title.as_str(), body), ("Delete stash", drop_confirm_body(&message)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pop_stash_without_stashes_says_so() {
    let (mut st, dir) = repo_with_topic("stash-confirm-none");
    assert!(palette(&mut st, C::PopStash).is_empty());
    assert_eq!(st.log.last().map(|e| e.text.as_str()), Some("There are no stashes."));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

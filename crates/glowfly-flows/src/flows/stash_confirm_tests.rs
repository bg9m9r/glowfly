//! Headless tests for the question before applying or popping a stash on top of uncommitted
//! changes: none on a clean tree, the Confirm on a dirty one once the stash's files are read
//! (sub-list and Pop stash entries alike) with the files both change, the job once it is
//! accepted, Pop stash naming the newest stash by position and commit, the refusal over
//! conflicts, and Delete's own question.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::Effect;
use crate::effect::StashEffect;
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

/// Runs palette command `cmd` on a dirty tree: nothing runs yet, the stash's files are read,
/// then the question opens.
fn ask(st: &mut AppState, cmd: C) {
    let fx = palette(st, cmd);
    assert!(matches!(&fx[..], [Effect::Stash(StashEffect::ReadPaths { .. })]), "reads the stash's files first");
    assert!(st.modal.is_none());
    finish(st, fx);
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
    // Pop stash names the stash the snapshot shows, so a stash pushed meanwhile isn't popped.
    let (cmds, expect) = git_job(&palette(&mut st, C::PopStash));
    assert_eq!((cmds, expect), (argv(&["stash", "pop", "stash@{0}"]), Some(("stash@{0}".into(), oid))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dirty_tree_asks_first() {
    let (mut st, dir, oid) = one_stash("stash-confirm-dirty");
    make_dirty(&mut st, &dir);
    let message = st.snap.as_ref().unwrap().stashes[0].message.clone();

    ask(&mut st, C::StashOp { action: StashAction::Apply, oid });
    assert!(st.palette.is_none(), "the palette closed before the question");
    let (title, body, button) = confirm(&st);
    assert_eq!((title.as_str(), button.as_str()), ("Apply stash", "Apply"));
    assert!(body.contains(&message) && body.contains("uncommitted changes"), "{body}");
    assert!(body.contains("a file you have changed too") && body.contains("\n    a.txt"), "the overlap: {body}");
    assert!(!body.contains("the stash is kept"));
    let (cmds, expect) = git_job(&run(&mut st, Msg::ConfirmAccepted));
    assert_eq!((cmds, expect), (argv(&["stash", "apply", "stash@{0}"]), Some(("stash@{0}".into(), oid))));
    st.job = None;

    ask(&mut st, C::StashOp { action: StashAction::Pop, oid });
    let (title, body, button) = confirm(&st);
    assert_eq!((title.as_str(), button.as_str()), ("Pop stash", "Pop"));
    assert!(body.contains(&message) && body.contains("the stash is kept"), "{body}");
    assert_eq!(git_job(&run(&mut st, Msg::ConfirmAccepted)).0, argv(&["stash", "pop", "stash@{0}"]));
    st.job = None;

    ask(&mut st, C::PopStash);
    assert_eq!(confirm(&st).0, "Pop stash", "the Pop stash entry asks too");
    let (cmds, expect) = git_job(&run(&mut st, Msg::ConfirmAccepted));
    assert_eq!((cmds, expect), (argv(&["stash", "pop", "stash@{0}"]), Some(("stash@{0}".into(), oid))));
    st.job = None;

    assert!(palette(&mut st, C::StashOp { action: StashAction::Drop, oid }).is_empty());
    let (title, body, _) = confirm(&st);
    assert_eq!((title.as_str(), body), ("Delete stash", drop_confirm_body(&message)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changes_apart_from_the_stash_leave_the_warning_out() {
    let (mut st, dir, oid) = one_stash("stash-confirm-apart");
    std::fs::write(dir.join("n.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().unwrap().is_dirty());
    ask(&mut st, C::StashOp { action: StashAction::Apply, oid });
    let (_, body, _) = confirm(&st);
    assert!(body.contains("none of the files you have changed"), "{body}");
    assert!(!body.contains("conflicts") && !body.contains("n.txt"), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stash_pushed_elsewhere_is_not_popped_instead() {
    let (mut st, dir, oid) = one_stash("stash-confirm-shifted");
    // A terminal pushes another stash after the snapshot: stash@{0} is now that one.
    std::fs::write(dir.join("t.txt"), "terminal\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "terminal"]);
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::PopStash)));
    let last = st.log.last().unwrap();
    assert!(!last.ok && last.text.contains("stash list changed"), "{}", last.text);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 2, "nothing popped");
    assert!(snap.stashes.iter().any(|s| s.oid == oid));
    assert!(snap.stashes[0].message.contains("terminal"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicts_refuse_apply_and_pop_without_a_question() {
    let (mut st, dir, oid) = one_stash("stash-confirm-conflicts");
    // A conflicting merge leaves a.txt conflicted in the index.
    git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("a.txt"), "side\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "side"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "main\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "main"]);
    let (merged, _) = glowfly_core::testkit::try_git_env(&dir, &["merge", "-q", "side"], &[]);
    assert!(!merged, "the merge stops on the conflict");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(crate::state::rebase::has_conflicts(st.snap.as_ref().unwrap()));
    for cmd in [C::StashOp { action: StashAction::Apply, oid }, C::StashOp { action: StashAction::Pop, oid }, C::PopStash] {
        st.log.clear();
        drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
        assert!(st.modal.is_none() && st.job.is_none(), "no question and no job");
        let last = st.log.last().unwrap();
        assert!(!last.ok && last.text.contains("conflicted"), "{}", last.text);
    }
    assert!(crate::state::toolbar_tips::pop_blocked(&st).is_some_and(|why| why.contains("conflicted")), "the toolbar's Pop is greyed");
    assert_eq!(st.snap.as_ref().unwrap().stashes.len(), 1);
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

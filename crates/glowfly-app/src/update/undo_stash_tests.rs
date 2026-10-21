//! Headless tests for undoing stashes and pops, and for the "Stash changes and undo" offer when
//! local changes block undoing a hard reset or Discard all.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, UndoResult};
use crate::msg::{CommitMsg, JobMsg, Msg, RepoMsg, StageMsg, TabsMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::palette::StashAction;
use glowfly_core::undo::{Action, DISCARD_STASH_MESSAGE, UNDO_STASH_MESSAGE, UndoKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap().replace("\r\n", "\n")
}

fn dirty(st: &AppState) -> bool {
    st.snap.as_ref().unwrap().is_dirty()
}

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().unwrap();
    snap.commits[snap.row_of[&snap.head_oid.unwrap()]].summary.clone()
}

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// Clicks the newest toast's action button.
fn click_toast_action(st: &mut AppState) {
    let id = st.toasts.last().unwrap().id;
    drive(st, Msg::ToastAction(id));
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn toolbar_stash_undo_and_redo() {
    let (mut st, dir) = opened("undo-stash-toolbar");
    drive(&mut st, Msg::git("Stash", &["stash", "push", "-u"]));
    assert!(!dirty(&st));
    assert!(matches!(st.undo.undo.last(), Some(Action::StashPush { .. })), "{:?}", st.undo.undo);
    assert!(st.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo stash 'WIP on main"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\n");
    assert!(st.snap.as_ref().unwrap().stashes.is_empty());
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!dirty(&st));
    let snap = st.snap.as_ref().unwrap();
    assert!(matches!(st.undo.undo.last(), Some(Action::StashPush { stash, .. }) if *stash == snap.stashes[0].oid));
    cleanup(&[&dir]);
}

#[test]
fn pop_on_a_clean_tree_is_undoable_and_on_a_dirty_one_is_not() {
    let (mut st, dir) = opened("undo-stash-pop");
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let oid = st.snap.as_ref().unwrap().stashes[0].oid;
    let pop = Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, oid });
    let fx = update(&mut st, pop, Instant::now());
    let want = UndoKind::StashApply { pop: true, rev: Some("stash@{0}".into()) };
    assert!(matches!(&fx[..], [Effect::RunGit { record: Some(k), .. }] if *k == want), "start_job recognises it: {fx:?}");
    super::tests::finish(&mut st, fx);
    assert!(dirty(&st));
    assert!(st.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo pop of stash 'On main: wip'"));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!((snap.stashes.len(), snap.stashes[0].oid), (1, oid), "the stash is back");

    // Onto local changes: the pop can't be told apart from them.
    std::fs::write(dir.join("b.txt"), "local\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, oid }));
    assert!(st.undo.undo.is_empty());
    let (_, why) = st.undo.undo_entry(st.settings.lang(), true);
    assert!(why.as_deref().is_some_and(|w| w.starts_with("Can't undo Pop stash")), "{why:?}");
    cleanup(&[&dir]);
}

/// A repository with "Second line" committed on "first", open.
fn committed(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.draft.summary = "Second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    (st, dir)
}

fn has_undo_stash(st: &AppState) -> bool {
    st.snap.as_ref().unwrap().stashes.iter().any(|s| s.message.ends_with(UNDO_STASH_MESSAGE))
}

#[test]
fn stash_changes_and_undo_a_hard_reset() {
    let (mut st, dir) = committed("undo-stash-offer-reset");
    let first = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "first").unwrap().oid.to_string();
    std::fs::write(dir.join("a.txt"), "before the reset\n").unwrap();
    drive(&mut st, Msg::git("Reset (hard)", &["reset", "--hard", &first]));
    std::fs::write(dir.join("a.txt"), "after the reset\n").unwrap();

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("Commit or stash"), "{}", t.text);
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Stash changes and undo"));
    click_toast_action(&mut st);
    assert_eq!(head_summary(&st), "Second line");
    assert_eq!(read(&dir, "a.txt"), "before the reset\n");
    assert!(has_undo_stash(&st));
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text.starts_with("Stash changes and undo hard reset")));
    assert!(st.log.iter().any(|l| l.text.contains("Your changes were stashed as \"Trellis: changes before undo\".")));
    cleanup(&[&dir]);
}

#[test]
fn stash_changes_and_undo_discard_all() {
    let (mut st, dir) = opened("undo-stash-offer-discard");
    drive(&mut st, Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]));
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(st.toasts.last().unwrap().action.as_ref().map(|a| a.label.as_str()), Some("Stash changes and undo"));
    click_toast_action(&mut st);
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\n");
    assert!(!dir.join("b.txt").exists(), "the new file went into the stash");
    assert!(has_undo_stash(&st));
    cleanup(&[&dir]);
}

#[test]
fn refusal_for_an_inactive_tab_offers_nothing() {
    let (a, b) = (scratch_repo("undo-stash-offer-a"), scratch_repo("undo-stash-offer-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]));
    std::fs::write(a.join("b.txt"), "new\n").unwrap();

    let effect =
        update(&mut st, Msg::Job(JobMsg::Undo), Instant::now()).into_iter().find(|e| matches!(e, Effect::UndoRedo { .. })).expect("undo");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    let reply = runner::execute(effect).expect("a result");
    assert!(matches!(&reply, Msg::Job(JobMsg::JobDone { undo: Some(UndoResult::Refused { offer_stash: true, .. }), .. })));
    update(&mut st, reply, Instant::now());
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.action.as_ref().is_none_or(|a| a.label == "Details"), "no stash offer");
    cleanup(&[&a, &b]);
}

/// `committed`, plus two stashes made with git ("first" at stash@{1}, "second" at stash@{0});
/// returns the oid of "first".
fn two_stashes(name: &str) -> (AppState, PathBuf, git2::Oid) {
    let (mut st, dir) = committed(name);
    std::fs::write(dir.join("a.txt"), "first\n").unwrap();
    git(&dir, &["stash", "push", "-q", "-m", "first"]);
    std::fs::write(dir.join("a.txt"), "second\n").unwrap();
    git(&dir, &["stash", "push", "-q", "-m", "second"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let first = st.snap.as_ref().unwrap().stashes[1].oid;
    (st, dir, first)
}

fn stash_oids(st: &AppState) -> Vec<git2::Oid> {
    st.snap.as_ref().unwrap().stashes.iter().map(|s| s.oid).collect()
}

#[test]
fn drop_from_the_sidebar_is_undoable() {
    let (mut st, dir, first) = two_stashes("undo-stash-drop");
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: 1, oid: first }));
    assert!(!stash_oids(&st).contains(&first));
    assert!(matches!(st.undo.undo.last(), Some(Action::StashDrop { stash, .. }) if *stash == first), "{:?}", st.undo.undo);
    assert!(st.undo.blocked.is_none());
    assert!(matches!(st.undo.undo.first(), Some(Action::Commit { .. })), "the commit is still under it: {:?}", st.undo.undo);
    let tip = st.undo.undo_entry(st.settings.lang(), true).0;
    assert_eq!(tip, "Undo drop of stash 'On main: first'");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let back = st.snap.as_ref().unwrap().stashes.iter().find(|s| s.oid == first).cloned().expect("the stash is back");
    assert_eq!(back.message, "On main: first");
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Undo drop of stash 'On main: first' succeeded"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!stash_oids(&st).contains(&first));
    assert!(matches!(st.undo.undo.last(), Some(Action::StashDrop { .. })));
    cleanup(&[&dir]);
}

#[test]
fn drop_redo_refused_when_dropped_elsewhere() {
    let (mut st, dir, first) = two_stashes("undo-stash-drop-elsewhere");
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: 1, oid: first }));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(stash_oids(&st).contains(&first));
    git(&dir, &["stash", "drop", "-q", "stash@{0}"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("The stash was dropped or applied since"), "{}", t.text);
    assert!(st.undo.undo.is_empty() && st.undo.redo.is_empty(), "{:?}", st.undo);
    cleanup(&[&dir]);
}

//! Headless tests for undoing stashes and pops, and for the "Stash changes and undo" offer when
//! local changes block undoing a hard reset, Discard all, a stash, or a merge, pull,
//! cherry-pick, revert or rebase (whose undo puts the changes back with their staged state).

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
    snap.commits[snap.row_of[&snap.head_oid.unwrap()]].summary.to_string()
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
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashPush { .. })), "{:?}", st.session.undo.undo);
    assert!(st.session.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo stash 'WIP on main"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\n");
    assert!(st.snap.as_ref().unwrap().stashes.is_empty());
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!dirty(&st));
    let snap = st.snap.as_ref().unwrap();
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashPush { stash, .. }) if *stash == snap.stashes[0].oid));
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
    assert!(st.session.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo pop of stash 'On main: wip'"));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!((snap.stashes.len(), snap.stashes[0].oid), (1, oid), "the stash is back");

    // Onto local changes: the pop can't be told apart from them.
    std::fs::write(dir.join("b.txt"), "local\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, oid }));
    assert!(st.session.undo.undo.is_empty());
    let (_, why) = st.session.undo.undo_entry(st.settings.lang(), true);
    assert!(why.as_deref().is_some_and(|w| w.starts_with("Can't undo Pop stash")), "{why:?}");
    cleanup(&[&dir]);
}

/// A repository with "Second line" committed on "first", open.
fn committed(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Second line".into();
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
    let t = st.toasts.last().unwrap();
    assert!(
        t.ok && t.text.starts_with("Undid hard reset") && t.text.ends_with("in the stash \"Glowfly: changes before undo\"."),
        "{}",
        t.text
    );
    assert!(st.log.iter().any(|l| l.text.contains("Your changes were stashed as \"Glowfly: changes before undo\".")));
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
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashDrop { stash, .. }) if *stash == first), "{:?}", st.session.undo.undo);
    assert!(st.session.undo.blocked.is_none());
    assert!(
        matches!(st.session.undo.undo.first(), Some(Action::Commit { .. })),
        "the commit is still under it: {:?}",
        st.session.undo.undo
    );
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert_eq!(tip, "Undo drop of stash 'On main: first'");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let back = st.snap.as_ref().unwrap().stashes.iter().find(|s| s.oid == first).cloned().expect("the stash is back");
    assert_eq!(back.message, "On main: first");
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Undo drop of stash 'On main: first' succeeded"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!stash_oids(&st).contains(&first));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::StashDrop { .. })));
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
    // Only the expired drop goes: the commit under it is checked again when it is undone.
    assert!(st.session.undo.redo.is_empty(), "{:?}", st.session.undo);
    assert!(matches!(&st.session.undo.undo[..], [Action::Commit { .. }]), "{:?}", st.session.undo);
    cleanup(&[&dir]);
}

#[test]
fn a_dropped_stash_survives_a_job_that_cant_be_undone() {
    let (mut st, dir, first) = two_stashes("undo-stash-drop-unrelated");
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: 1, oid: first }));
    drive(&mut st, Msg::git("Rename branch", &["branch", "-m", "main", "trunk"]));
    assert_eq!(st.session.undo.blocked.as_deref(), Some("Rename branch"));
    assert!(
        matches!(&st.session.undo.undo[..], [Action::StashDrop { stash, .. }] if *stash == first),
        "the commit under it can't be undone past the rename; the drop can: {:?}",
        st.session.undo.undo
    );

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let back = st.snap.as_ref().unwrap().stashes.iter().find(|s| s.oid == first).cloned().expect("the stash is back");
    assert_eq!(back.message, "On main: first");
    cleanup(&[&dir]);
}

/// Undoes the newest entry, refused for local changes, then presses the toast's Stash changes
/// and undo.
fn stash_and_undo(st: &mut AppState) {
    drive(st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("Commit or stash"), "{}", t.text);
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Stash changes and undo"));
    click_toast_action(st);
}

#[test]
fn stash_changes_and_undo_a_stash_leaves_the_newer_changes_stashed() {
    let (mut st, dir) = opened("undo-stash-offer-stash");
    drive(&mut st, Msg::git("Stash", &["stash", "push", "-u"]));
    std::fs::write(dir.join("a.txt"), "newer\n").unwrap();
    stash_and_undo(&mut st);
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\n", "the stash is back");
    assert!(has_undo_stash(&st), "the newer changes are in their own stash");
    let t = st.toasts.last().unwrap();
    assert!(t.ok && t.text.starts_with("Undid stash 'WIP on main"), "{}", t.text);
    cleanup(&[&dir]);
}

/// l.txt committed on main ("base"), then feature adds f.txt ("on feature") and main m.txt
/// ("on main"); opened on main, clean.
fn forked(name: &str) -> (AppState, PathBuf) {
    let dir = super::tests::scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    let commit = |file: &str, msg: &str| {
        std::fs::write(dir.join(file), format!("{file}\n")).unwrap();
        git(&dir, &["add", file]);
        git(&dir, &["commit", "-q", "-m", msg]);
    };
    commit("l.txt", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit("f.txt", "on feature");
    git(&dir, &["checkout", "-q", "main"]);
    commit("m.txt", "on main");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// Runs `op` on main, makes changes of every kind to files it doesn't touch (l.txt staged with
/// an unstaged edit on top, the untracked new.txt), then stashes them and undoes it: HEAD is back
/// on "on main" and the changes are back exactly, staged part too, with no stash left.
fn stash_and_undo_a_move(name: &str, op: Msg, label: &str) {
    let (mut st, dir) = forked(name);
    drive(&mut st, op);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { .. })), "{:?}", st.session.undo.undo);
    std::fs::write(dir.join("l.txt"), "staged\n").unwrap();
    git(&dir, &["add", "l.txt"]);
    std::fs::write(dir.join("l.txt"), "staged\nunstaged\n").unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    stash_and_undo(&mut st);

    assert_eq!(head_summary(&st), "on main");
    assert_eq!(read(&dir, "l.txt"), "staged\nunstaged\n");
    let repo = git2::Repository::open(&dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new("l.txt"), 0).unwrap();
    assert_eq!(repo.find_blob(entry.id).unwrap().content(), b"staged\n", "the staged part is staged again");
    assert_eq!(read(&dir, "new.txt"), "new\n");
    assert!(!has_undo_stash(&st), "popped");
    let t = st.toasts.last().unwrap();
    assert!(t.ok && t.text == format!("Stash changes and undo {label} succeeded"), "{}", t.text);
    assert!(matches!(st.session.undo.redo.last(), Some(Action::Moved { .. })), "{:?}", st.session.undo);
    cleanup(&[&dir]);
}

#[test]
fn stash_changes_and_undo_a_merge() {
    stash_and_undo_a_move("undo-stash-offer-merge", Msg::git("Merge", &["merge", "--no-edit", "feature"]), "merge of feature");
}

#[test]
fn stash_changes_and_undo_a_pull() {
    let pull = Msg::git("Pull", &["pull", "--no-rebase", "--no-edit", ".", "feature"]);
    stash_and_undo_a_move("undo-stash-offer-pull", pull, "pull");
}

#[test]
fn stash_changes_and_undo_a_cherry_pick() {
    stash_and_undo_a_move("undo-stash-offer-pick", Msg::git("Cherry-pick", &["cherry-pick", "feature"]), "cherry-pick of feature");
}

#[test]
fn stash_changes_and_undo_a_revert() {
    stash_and_undo_a_move("undo-stash-offer-revert", Msg::git("Revert", &["revert", "--no-edit", "main"]), "revert of main");
}

#[test]
fn stash_changes_and_undo_a_rebase() {
    stash_and_undo_a_move("undo-stash-offer-rebase", Msg::git("Rebase", &["rebase", "feature"]), "rebase onto feature");
}

#[test]
fn changes_that_cant_go_back_cleanly_stay_in_the_stash() {
    let (mut st, dir) = forked("undo-stash-offer-kept");
    drive(&mut st, Msg::git("Merge", &["merge", "--no-edit", "feature"]));
    // f.txt came with the merge, so undoing it deletes the file this edit is in.
    std::fs::write(dir.join("f.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    stash_and_undo(&mut st);
    assert_eq!(head_summary(&st), "on main", "the undo ran");
    assert!(has_undo_stash(&st), "the stash is kept");
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("came back with conflicts in 1 file") && t.text.contains(UNDO_STASH_MESSAGE), "{}", t.text);
    assert!(st.log.iter().any(|l| l.text.contains("nothing was dropped")));
    assert!(matches!(st.session.undo.redo.last(), Some(Action::Moved { .. })), "{:?}", st.session.undo);
    cleanup(&[&dir]);
}

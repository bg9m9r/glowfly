//! Headless tests for the palette's stash steps: when the stash entries are offered, the
//! apply, pop and delete sub-lists (filtering, running the chosen stash, following it by oid
//! when the stack changes) and the stash message step.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::palette::{InputKind, Items, Mode, Pick, PickKind, StashAction, row_count, shown};
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;

const WITH_MESSAGE: &str = "Stash with message…";
const CHOOSERS: [&str; 3] = ["Apply stash…", "Pop stash…", "Delete stash…"];

use crate::state::palette::PaletteCommand as C;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn labels(st: &AppState, query: &str) -> Vec<String> {
    shown(st, query).into_iter().map(|s| s.entry.label).collect()
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn set_query(st: &mut AppState, q: &str) {
    st.palette.as_mut().unwrap().query = q.into();
}

/// The labels of the shown rows, after ranking for the current query.
fn rows(st: &mut AppState) -> Vec<String> {
    row_count(st);
    let pk = pick(st);
    pk.ranked().unwrap().hits.iter().map(|h| pk.label(h.index).to_owned()).collect()
}

fn choose(st: &mut AppState, action: StashAction) -> Vec<Effect> {
    run(st, Msg::RunPaletteCommand(C::ChooseStash(action)))
}

/// The command for the first row matching `query` in the open sub-list.
fn command_for(st: &mut AppState, query: &str) -> C {
    set_query(st, query);
    row_count(st);
    let pk = pick(st);
    pk.command(pk.ranked().unwrap().hits[0].index).unwrap()
}

/// The argv of the one RunGit effect in `fx`.
fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("expected one RunGit, got {} effects", other.len()),
    }
}

fn argv(a: &[&str]) -> Vec<Vec<String>> {
    vec![a.iter().map(|s| s.to_string()).collect()]
}

/// A clean repository with stashes [0] "On main: second" and [1] "On main: first"; "second"
/// holds the untracked c.txt.
pub(super) fn two_stashes(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = repo_with_topic(name);
    git(&dir, &["stash", "push", "-u", "-m", "first"]);
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "second"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!(snap.stashes.len(), 2);
    assert!(snap.stashes[0].message.contains(": second"));
    assert!(snap.stashes[1].message.contains(": first"));
    (st, dir)
}

#[test]
fn stash_entries_availability() {
    let st = state();
    let l = labels(&st, "");
    assert!(!l.iter().any(|x| x == WITH_MESSAGE || CHOOSERS.contains(&x.as_str())), "no repository");

    let (st, dir) = repo_with_topic("stash-entries-dirty");
    let l = labels(&st, "");
    let at = l.iter().position(|x| x == "Stash changes").unwrap();
    assert_eq!(l[at + 1], WITH_MESSAGE);
    assert!(!l.iter().any(|x| CHOOSERS.contains(&x.as_str())), "no stashes yet");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = two_stashes("stash-entries-two");
    let entries = shown(&st, "");
    assert!(!entries.iter().any(|s| s.entry.label == WITH_MESSAGE), "clean tree");
    let at = entries.iter().position(|s| s.entry.label == "Pop stash").unwrap();
    for (i, want) in CHOOSERS.iter().enumerate() {
        let e = &entries[at + 1 + i].entry;
        assert_eq!(e.label, *want);
        assert_eq!(e.detail.as_deref(), Some("2 stashes"));
    }
    assert!(labels(&st, "drop").iter().any(|x| x == "Delete stash…"));

    git(&dir, &["stash", "drop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let one = shown(&st, "");
    let apply = one.iter().find(|s| s.entry.label == "Apply stash…").unwrap();
    assert_eq!(apply.entry.detail.as_deref(), Some("1 stash"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn choose_stash_enters_sub_list() {
    let (mut st, dir) = two_stashes("stash-choose");
    let msgs: Vec<String> = st.snap.as_ref().unwrap().stashes.iter().map(|s| s.message.clone()).collect();
    run(&mut st, Msg::TogglePalette);
    set_query(&mut st, "ap");
    assert!(choose(&mut st, StashAction::Apply).is_empty());
    let p = st.palette.as_ref().expect("still open");
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);
    assert_eq!(pick(&st).kind, PickKind::Stash(StashAction::Apply));
    assert_eq!(pick(&st).items, Items::Ready(msgs));
    assert_eq!(pick(&st).detail(0).as_deref(), Some("stash@{0}"));
    assert_eq!(pick(&st).detail(1).as_deref(), Some("stash@{1}"));

    let texts = |a| (PickKind::Stash(a).title(), PickKind::Stash(a).prompt());
    assert_eq!(texts(StashAction::Apply), ("Apply stash", "Find a stash to apply"));
    assert_eq!(texts(StashAction::Pop), ("Pop stash", "Find a stash to pop"));
    assert_eq!(texts(StashAction::Drop), ("Delete stash", "Find a stash to delete"));

    // Closed: entering opens the palette.
    st.palette = None;
    choose(&mut st, StashAction::Pop);
    assert_eq!(pick(&st).kind, PickKind::Stash(StashAction::Pop));

    // Not under a dialog.
    st.palette = None;
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    assert!(choose(&mut st, StashAction::Apply).is_empty());
    assert!(st.palette.is_none());
    let _ = std::fs::remove_dir_all(&dir);

    // No stashes: nothing happens.
    let (mut st, dir) = repo_with_topic("stash-choose-none");
    run(&mut st, Msg::TogglePalette);
    choose(&mut st, StashAction::Apply);
    assert!(matches!(st.palette.as_ref().unwrap().mode, Mode::Commands));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stash_sub_list_filters() {
    let (mut st, dir) = two_stashes("stash-filter");
    choose(&mut st, StashAction::Apply);
    assert_eq!(row_count(&mut st), 2);

    set_query(&mut st, "first");
    let first = rows(&mut st);
    assert_eq!(first.len(), 1);
    assert!(first[0].contains(": first"));

    // The ref matches as a hidden keyword, without highlighting the label.
    set_query(&mut st, "stash@{1}");
    assert_eq!(rows(&mut st), first);
    assert!(pick(&st).ranked().unwrap().hits[0].matched.is_empty());

    set_query(&mut st, "zzz");
    assert_eq!(row_count(&mut st), 0);

    set_query(&mut st, "");
    run(&mut st, Msg::PaletteMove(-1));
    assert_eq!(st.palette.as_ref().unwrap().selected, 1, "wraps to the last row");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_chosen_stash() {
    let (mut st, dir) = two_stashes("stash-apply");
    choose(&mut st, StashAction::Apply);
    let cmd = command_for(&mut st, "first");
    assert!(matches!(cmd, C::StashOp { action: StashAction::Apply, .. }));

    // The argv, before running it for real.
    let (mut fresh, fresh_dir) = two_stashes("stash-apply-argv");
    let oid = fresh.snap.as_ref().unwrap().stashes[1].oid;
    let fx = run(&mut fresh, Msg::RunPaletteCommand(C::StashOp { action: StashAction::Apply, oid }));
    assert_eq!(git_cmds(&fx), argv(&["stash", "apply", "stash@{1}"]));
    let _ = std::fs::remove_dir_all(&fresh_dir);

    drive(&mut st, Msg::RunPaletteCommand(cmd));
    assert!(st.palette.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 2, "apply keeps the stash");
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt" && f.status == 'M'));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pop_chosen_stash() {
    let (mut st, dir) = two_stashes("stash-pop");
    choose(&mut st, StashAction::Pop);
    let cmd = command_for(&mut st, "second");
    drive(&mut st, Msg::RunPaletteCommand(cmd));
    assert!(st.palette.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.stashes[0].message.contains("first"));
    assert!(snap.unstaged.iter().any(|f| f.path == "c.txt" && f.status == '?'));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_chosen_stash_confirms() {
    let (mut st, dir) = two_stashes("stash-drop");
    choose(&mut st, StashAction::Drop);
    let cmd = command_for(&mut st, "first");
    let fx = run(&mut st, Msg::RunPaletteCommand(cmd));
    assert!(st.palette.is_none());
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })));
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("no confirmation") };
    assert_eq!(title, "Delete stash");
    assert_eq!(button, "Delete");
    assert!(body.contains("On main: first") && body.contains("cannot be undone"), "{body}");
    let first = st.snap.as_ref().unwrap().stashes[1].oid;
    assert!(
        matches!(then.as_ref(), Msg::Stash { action: StashAction::Drop, index: 1, oid } if *oid == first),
        "the confirmation doesn't drop the stash"
    );
    drive(&mut st, Msg::ConfirmAccepted);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.stashes[0].message.contains("second"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stash_op_follows_oid_when_list_changes() {
    let (mut st, dir) = two_stashes("stash-oid");
    choose(&mut st, StashAction::Apply);
    let cmd = pick(&st).command(0).unwrap();

    // Another stash pushes "second" down to stash@{1}.
    std::fs::write(dir.join("d.txt"), "d\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "third"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = run(&mut st, Msg::RunPaletteCommand(cmd.clone()));
    assert_eq!(git_cmds(&fx), argv(&["stash", "apply", "stash@{1}"]));
    // Only the argv matters here; the job never ran.
    st.job = None;

    // Dropped elsewhere: nothing runs and the log says so.
    git(&dir, &["stash", "drop", "stash@{1}"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    choose(&mut st, StashAction::Apply);
    let before = st.log.len();
    let fx = run(&mut st, Msg::RunPaletteCommand(cmd));
    assert!(fx.is_empty());
    assert!(st.palette.is_none());
    assert_eq!(st.log.len(), before + 1);
    let last = st.log.last().unwrap();
    assert!(!last.ok && last.text.contains("no longer exists"), "{}", last.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stash_message_step() {
    let (mut st, dir) = repo_with_topic("stash-message");
    run(&mut st, Msg::TogglePalette);
    set_query(&mut st, "msg");
    assert!(run(&mut st, Msg::RunPaletteCommand(C::StashMessage)).is_empty());
    let p = st.palette.as_ref().unwrap();
    assert!(matches!(p.mode, Mode::Input(InputKind::StashMessage)));
    assert!(p.query.is_empty());

    run(&mut st, Msg::PaletteMove(1));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    assert_eq!(row_count(&mut st), 0);

    assert!(run(&mut st, Msg::RunPaletteCommand(C::StashWithMessage("   ".into()))).is_empty());
    assert!(matches!(st.palette.as_ref().unwrap().mode, Mode::Input(_)), "a blank message leaves the step open");

    let typed = || Msg::RunPaletteCommand(C::StashWithMessage("  Half-done login  ".into()));
    let fx = run(&mut st, typed());
    assert_eq!(git_cmds(&fx), argv(&["stash", "push", "-u", "-m", "Half-done login"]));
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = repo_with_topic("stash-message-drive");
    run(&mut st, Msg::RunPaletteCommand(C::StashMessage));
    drive(&mut st, typed());
    assert!(st.palette.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert!(snap.stashes[0].message.ends_with("Half-done login"), "{}", snap.stashes[0].message);

    // Clean now: the step doesn't open.
    run(&mut st, Msg::RunPaletteCommand(C::StashMessage));
    assert!(st.palette.is_none());
    assert_eq!(InputKind::StashMessage.command(" "), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn back_from_stash_steps() {
    let (mut st, dir) = two_stashes("stash-back");
    choose(&mut st, StashAction::Pop);
    set_query(&mut st, "x");
    st.palette.as_mut().unwrap().selected = 1;
    run(&mut st, Msg::PaletteBack);
    let p = st.palette.as_ref().unwrap();
    assert!(matches!(p.mode, Mode::Commands));
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);

    // A file list arriving for the current generation doesn't replace the stashes.
    choose(&mut st, StashAction::Pop);
    let before = match &pick(&st).items {
        Items::Ready(v) => v.clone(),
        _ => panic!("stashes not ready"),
    };
    let generation = st.palette_gen;
    run(&mut st, Msg::TrackedPathsLoaded { generation, result: Ok(vec!["a.txt".into()]) });
    assert_eq!(pick(&st).items, Items::Ready(before));
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = repo_with_topic("stash-back-input");
    run(&mut st, Msg::RunPaletteCommand(C::StashMessage));
    set_query(&mut st, "x");
    run(&mut st, Msg::PaletteBack);
    let p = st.palette.as_ref().unwrap();
    assert!(matches!(p.mode, Mode::Commands));
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

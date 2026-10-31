//! Headless tests for the WIP lists' keys: Up/Down between the Unstaged or Staged Files, S and
//! U on a file still opening, moving on to the neighbouring file after S or U, and a refresh
//! asking again for a file still opening.

use super::shortcuts_tests::one_hunk;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::{DiffMsg, Msg, RepoMsg, StageMsg, TabsMsg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::wip_lists::MoveOn;
use glowfly_core::repo::{self, DiffTarget, FileEntry, LoadOptions};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository with a.txt modified and b.txt and c.txt new (untracked), loaded.
fn three_files(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    let mut st = state();
    reload(&mut st, &dir);
    (st, dir)
}

fn reload(st: &mut AppState, dir: &Path) {
    st.snap = Some(repo::load(dir, &LoadOptions::all(100)).unwrap());
}

fn un(path: &str) -> DiffTarget {
    DiffTarget::Unstaged(path.into())
}

fn sd(path: &str) -> DiffTarget {
    DiffTarget::Staged(path.into())
}

/// Puts `one_hunk` on screen as `t`, as if its diff had arrived.
fn show(st: &mut AppState, t: DiffTarget) {
    st.session.diff.view = Some((t, one_hunk().into()));
    st.session.diff.shown_gen = st.session.diff.generation;
}

fn key(st: &mut AppState, action: KeyAction) -> Vec<Effect> {
    update(st, Msg::Shortcut(action), Instant::now())
}

fn step(st: &mut AppState, staged: bool, down: bool) -> Vec<Effect> {
    key(st, KeyAction::WipList { staged, key: ListKey::Move(ListMove::Step { down }) })
}

/// The diffs `fx` asks for, with whether each is a reload.
fn loads(fx: &[Effect]) -> Vec<(DiffTarget, bool)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadDiff { target, reload, .. } => Some((target.clone(), *reload)),
            _ => None,
        })
        .collect()
}

/// `fx` is exactly one fresh open of `t`, and the list will reveal it.
#[track_caller]
fn opens(st: &AppState, fx: &[Effect], t: DiffTarget) {
    assert!(matches!(fx, [Effect::LoadDiff { reload: false, .. }]), "{fx:?}");
    assert_eq!(loads(fx), vec![(t.clone(), false)]);
    assert_eq!(st.view.diff_opening.as_ref(), Some(&t));
    assert_eq!(st.view.file_reveal.as_ref().map(|(r, _)| r), Some(&t));
}

fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::LocalOp { op, .. } => Some(op),
        _ => None,
    })
}

#[track_caller]
fn all_ok(st: &AppState) {
    assert!(st.log.iter().all(|l| l.ok), "log: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
}

#[test]
fn down_and_up_step_through_the_unstaged_list() {
    let (mut st, dir) = three_files("wip-keys-step");
    show(&mut st, un("a.txt"));
    let fx = step(&mut st, false, true);
    opens(&st, &fx, un("b.txt"));
    // Before b.txt arrives: the step goes on from the file still loading.
    let fx = step(&mut st, false, true);
    opens(&st, &fx, un("c.txt"));
    assert!(step(&mut st, false, true).is_empty(), "nothing below the last file");
    let fx = step(&mut st, false, false);
    opens(&st, &fx, un("b.txt"));
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_step_with_nothing_open_starts_at_the_first_file() {
    let (mut st, dir) = three_files("wip-keys-first");
    let fx = step(&mut st, false, false);
    opens(&st, &fx, un("a.txt"));
    let mut st2 = state();
    reload(&mut st2, &dir);
    let fx = step(&mut st2, false, true);
    opens(&st2, &fx, un("a.txt"));
    assert!(step(&mut st2, true, true).is_empty(), "the Staged list is empty");
    assert!(step(&mut st2, true, false).is_empty(), "the Staged list is empty");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steps_stay_in_their_list() {
    let (mut st, dir) = three_files("wip-keys-lists");
    git(&dir, &["add", "b.txt"]);
    reload(&mut st, &dir);
    show(&mut st, un("c.txt"));
    assert!(step(&mut st, false, true).is_empty(), "the last unstaged file never steps into Staged");
    // The open file isn't in the Staged list, so a step there starts at its first file.
    let fx = step(&mut st, true, true);
    opens(&st, &fx, sd("b.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steps_skip_conflicted_files() {
    let (mut st, dir) = three_files("wip-keys-conflicted");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.retain(|f| f.path != "b.txt");
    snap.unstaged.insert(1, FileEntry::new("b.txt", '!'));
    show(&mut st, un("a.txt"));
    let fx = step(&mut st, false, true);
    opens(&st, &fx, un("c.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steps_are_ignored_under_preferences_or_a_credential_prompt() {
    let (mut st, dir) = three_files("wip-keys-covered");
    show(&mut st, un("a.txt"));
    st.prefs = Some(crate::state::PrefsState::new(crate::state::PrefPage::General));
    assert!(step(&mut st, false, true).is_empty());
    assert!(st.view.diff_opening.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_acts_on_the_file_still_loading() {
    let (mut st, dir) = three_files("wip-keys-s-loading");
    show(&mut st, un("a.txt"));
    assert!(update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Only }), Instant::now()).is_empty());
    assert!(!st.view.diff_lines.is_empty());
    step(&mut st, false, true);
    let fx = key(&mut st, KeyAction::Stage);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("b.txt".into())), "{fx:?}");
    assert_eq!(st.view.stage_move_on.as_ref().map(|m| &m.from), Some(&un("b.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_on_selected_lines_does_not_move_on() {
    let (mut st, dir) = three_files("wip-keys-s-lines");
    show(&mut st, un("a.txt"));
    assert!(update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Only }), Instant::now()).is_empty());
    let fx = key(&mut st, KeyAction::Stage);
    assert!(matches!(local_op(&fx), Some(LocalOp::Lines { .. })), "{fx:?}");
    assert!(st.view.stage_move_on.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_moves_on_to_the_next_unstaged_file_end_to_end() {
    let dir = scratch_repo("wip-keys-s-e2e");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("b.txt"))));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&un("b.txt")));
    // The status arrives before c.txt's diff, which a refresh would otherwise supersede.
    drive(&mut st, Msg::Shortcut(KeyAction::Stage));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().any(|f| f.path == "b.txt"), "b.txt is staged");
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&un("c.txt")));
    // The last file: the one above it opens.
    drive(&mut st, Msg::Shortcut(KeyAction::Stage));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&un("a.txt")));
    // The only file left: nothing opens, and the reload closes its emptied diff.
    drive(&mut st, Msg::Shortcut(KeyAction::Stage));
    assert!(st.session.diff.view.is_none(), "{:?}", st.session.diff.view.as_ref().map(|(t, _)| t));
    assert!(st.snap.as_ref().unwrap().unstaged.is_empty());
    assert_eq!(st.view.diff_opening, None);
    assert!(st.view.stage_move_on.is_none());
    all_ok(&st);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn u_moves_on_in_the_staged_list_end_to_end() {
    let dir = scratch_repo("wip-keys-u-e2e");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    git(&dir, &["add", "."]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("a.txt"))));
    drive(&mut st, Msg::Shortcut(KeyAction::Unstage));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&sd("b.txt")));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    // The last staged file, with b.txt above it.
    drive(&mut st, Msg::Shortcut(KeyAction::Unstage));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&sd("b.txt")));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.staged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["b.txt"]);
    assert!(snap.unstaged.iter().any(|f| f.path == "c.txt" && f.status == '?'), "c.txt is untracked again");
    assert_eq!(st.view.diff_opening, None);
    all_ok(&st);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_stage_stays_on_the_file() {
    let (mut st, dir) = three_files("wip-keys-failed");
    show(&mut st, un("b.txt"));
    st.view.stage_move_on = Some(MoveOn { from: un("b.txt"), to: Some(un("c.txt")) });
    let fx = update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Err("no".into()))), Instant::now());
    assert!(loads(&fx).is_empty(), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    assert!(st.view.stage_move_on.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moving_away_before_the_stage_finishes_keeps_that_file() {
    let (mut st, dir) = three_files("wip-keys-away");
    show(&mut st, un("b.txt"));
    let fx = key(&mut st, KeyAction::Stage);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("b.txt".into())));
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))), Instant::now());
    let fx = update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))), Instant::now());
    assert!(loads(&fx).is_empty(), "{fx:?}");
    assert_eq!(st.view.diff_opening, Some(un("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_status_read_while_a_file_opens_asks_for_that_file() {
    let (mut st, dir) = three_files("wip-keys-status");
    show(&mut st, un("a.txt"));
    let fx = update(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("b.txt"))), Instant::now());
    assert_eq!(loads(&fx), vec![(un("b.txt"), false)]);
    let fx = update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))), Instant::now());
    let status =
        fx.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).and_then(crate::runner::testing::execute).expect("a status read");
    let fx = update(&mut st, status, Instant::now());
    assert_eq!(loads(&fx), vec![(un("b.txt"), false)], "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopening_the_diff_while_a_file_opens_reopens_that_file() {
    let (mut st, dir) = three_files("wip-keys-reopen");
    show(&mut st, un("a.txt"));
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("b.txt"))), Instant::now());
    let fx = update(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)), Instant::now());
    assert_eq!(loads(&fx), vec![(un("b.txt"), false)], "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_still_opening_that_left_its_list_is_dropped() {
    let (mut st, dir) = three_files("wip-keys-left");
    show(&mut st, un("a.txt"));
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("x.txt"))), Instant::now());
    let fx = update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))), Instant::now());
    let status =
        fx.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).and_then(crate::runner::testing::execute).expect("a status read");
    let fx = update(&mut st, status, Instant::now());
    assert_eq!(loads(&fx), vec![(un("a.txt"), true)], "{fx:?}");
    assert_eq!(st.view.diff_opening, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parking_a_tab_drops_a_pending_open_and_move_on() {
    let dir = scratch_repo("wip-keys-park");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))), Instant::now());
    st.view.stage_move_on = Some(MoveOn { from: un("a.txt"), to: None });
    assert!(st.view.diff_opening.is_some());
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.view.diff_opening, None);
    assert!(st.view.stage_move_on.is_none());
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.view.stage_move_on.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

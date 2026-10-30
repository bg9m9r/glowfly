//! Headless tests for moving a worktree: the refusals (main, this tab's, missing, locked), the
//! typed target's problems, a move through the runner and what follows it (an inactive tab, the
//! recent list, the closed tabs, favorites, repository groups and the saved tabs point at the new folder), a taken
//! target reopening the dialog with the error, and Browse… filling the dialog.

use super::tests::{drive, finish, git, same_place, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::WorktreeEffect;
use crate::msg::{FolderPurpose, Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::runner;
use crate::state::repo_groups::members;
use crate::state::tabs::ClosedTab;
use crate::state::worktree_move::{MoveForm, target_problem};
use crate::state::worktrees::find;
use crate::state::{AppState, Modal};
use crate::worktree::WorktreeMsg;
use glowfly_core::paths;
use glowfly_core::settings::RepoGroup;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn wt(m: WorktreeMsg) -> Msg {
    Msg::Worktree(m)
}

/// A repository opened in `st` with a linked worktree on `side` in `<repo>-worktrees/side`.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    let side = root.join("side");
    git(&dir, &["worktree", "add", "-q", "-b", "side", &side.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, side)
}

fn cleanup(dir: &Path, side: &Path) {
    let _ = std::fs::remove_dir_all(side.parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn form(st: &mut AppState) -> &mut MoveForm {
    match st.modal.as_mut() {
        Some(Modal::MoveWorktree(f)) => f,
        _ => panic!("the Move worktree dialog is not open: {:?}", st.log.last().map(|l| &l.text)),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).expect("a log entry")
}

/// Sends the dialog's Move, runs the effect and hands its result to `update`; returns what that
/// returned.
fn submit(st: &mut AppState) -> Vec<Effect> {
    let fx = update(st, wt(WorktreeMsg::Move), Instant::now());
    let [Effect::Worktree(WorktreeEffect::Move(_))] = &fx[..] else { panic!("{fx:?} {:?}", st.log.last().map(|l| &l.text)) };
    let moved = runner::execute(fx.into_iter().next().unwrap()).unwrap();
    update(st, moved, Instant::now())
}

#[test]
fn refusals() {
    let (mut st, dir, side) = opened("wtmv-refused");
    let gone = side.with_file_name("gone");
    let locked = side.with_file_name("locked");
    git(&dir, &["worktree", "add", "-q", "-b", "gone", &gone.display().to_string()]);
    git(&dir, &["worktree", "add", "-q", "-b", "locked", &locked.display().to_string()]);
    git(&dir, &["worktree", "lock", &locked.display().to_string()]);
    std::fs::remove_dir_all(&gone).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    for (path, why) in
        [(&dir, "The main worktree can't be moved."), (&gone, "Its folder is missing."), (&locked, "It is locked. Unlock it first.")]
    {
        drive(&mut st, wt(WorktreeMsg::MoveDialog(path.clone())));
        assert!(st.modal.is_none());
        assert!(last_log(&st).ends_with(why), "{}", last_log(&st));
    }
    drive(&mut st, wt(WorktreeMsg::Open(side.clone())));
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    assert!(st.modal.is_none() && last_log(&st).ends_with("This worktree is open in this tab."));
    git(&dir, &["worktree", "unlock", &locked.display().to_string()]);
    cleanup(&dir, &side);
}

#[test]
fn target_problems() {
    let (mut st, dir, side) = opened("wtmv-targets");
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    assert!(same_place(&PathBuf::from(&form(&mut st).folder), &side), "prefilled with where it is");
    let snap = st.snap.as_ref().unwrap();
    let problem = |p: &Path| target_problem(snap, &side, &p.display().to_string());
    assert_eq!(problem(&side).as_deref(), Some("That is where it is now."));
    assert_eq!(problem(&side.join("sub")).as_deref(), Some("A worktree can't be moved into its own folder."));
    assert!(problem(&dir).is_some_and(|p| p.starts_with("A worktree is already registered at ")));
    assert_eq!(target_problem(snap, &side, "rel/dir").as_deref(), Some("Enter the full path of the folder."));
    assert_eq!(problem(&side.with_file_name("elsewhere")), None);
    cleanup(&dir, &side);
}

#[test]
fn a_move_retargets_tabs_and_lists() {
    let (mut st, dir, side) = opened("wtmv-move");
    drive(&mut st, wt(WorktreeMsg::Open(side.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.tabs[1].parked.as_ref().is_some_and(|p| p.snap.is_some()));
    st.settings.favorite_repos = vec![paths::encode(&side)];
    st.settings.repo_groups = vec![grouping(&[&side])];
    st.closed_tabs = vec![ClosedTab { path: side.clone(), label: Some("Side work".into()) }];
    let to = side.with_file_name("deeper").join("renamed");
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    form(&mut st).folder = to.display().to_string();

    let fx = submit(&mut st);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveTabs { .. })), "the tabs are saved");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveRecent(_))));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    finish(&mut st, fx);
    assert!(st.modal.is_none());
    assert!(!side.exists() && to.join("a.txt").exists());
    let toast = st.toasts.last().unwrap();
    assert!(toast.ok && toast.text.starts_with("Moved worktree side to "), "{}", toast.text);
    let tab = &st.tabs[1];
    assert_eq!((tab.path.as_deref(), tab.name.as_str(), tab.real.as_deref()), (Some(to.as_path()), "renamed", None));
    assert!(tab.parked.as_ref().is_some_and(|p| p.snap.is_none()), "loads again when selected");
    assert!(st.recent.iter().any(|p| p == &to) && !st.recent.iter().any(|p| same_place(p, &side)), "{:?}", st.recent);
    assert_eq!(st.settings.favorite_repos, [paths::encode(&to)]);
    assert_eq!(st.settings.repo_groups[0].repos, [paths::encode(&to)], "its group follows it");
    assert_eq!(members(&st.settings, 0), [paths::decode(&paths::encode(&to))], "and still shows it");
    let closed: Vec<_> = st.closed_tabs.iter().map(|c| (c.path.clone(), c.label.as_deref())).collect();
    assert_eq!(closed, [(to.clone(), Some("Side work"))], "the closed tab keeps its label");
    assert!(st.snap.as_ref().unwrap().worktrees.iter().any(|w| same_place(&w.path, &to)));

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, &to), "the tab loads the new folder");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let _ = std::fs::remove_dir_all(side.with_file_name("deeper"));
    cleanup(&dir, &side);
}

/// A group called Work listing `folders`.
fn grouping(folders: &[&Path]) -> RepoGroup {
    let mut g = RepoGroup::new("Work", None);
    g.repos = folders.iter().map(|f| paths::encode(f)).collect();
    g
}

#[test]
fn a_move_onto_a_favorite_keeps_it_in_the_group() {
    let (mut st, dir, side) = opened("wtmv-grouped");
    let to = side.with_file_name("renamed");
    // The target is a favorite already, spelled with a trailing separator, and listed first,
    // so that spelling is the one the favorites keep.
    let other = format!("{}/", paths::encode(&to));
    st.settings.favorite_repos = vec![other.clone(), paths::encode(&side)];
    st.settings.repo_groups = vec![grouping(&[&side])];
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    form(&mut st).folder = to.display().to_string();
    let fx = submit(&mut st);
    finish(&mut st, fx);
    assert!(to.join("a.txt").exists());
    assert_eq!(st.settings.favorite_repos, std::slice::from_ref(&other));
    assert_eq!(st.settings.repo_groups[0].repos, [other], "the group names the favorite that stayed");
    assert_eq!(members(&st.settings, 0).len(), 1, "so it isn't dropped from the group");
    cleanup(&dir, &side);
}

#[test]
fn a_taken_target_reopens_the_dialog() {
    let (mut st, dir, side) = opened("wtmv-taken");
    let taken = side.with_file_name("taken");
    std::fs::create_dir_all(&taken).unwrap();
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    form(&mut st).folder = taken.display().to_string();
    let fx = submit(&mut st);
    finish(&mut st, fx);
    let f = form(&mut st);
    let error = f.current_error().expect("the error under the field").to_owned();
    assert!(error.starts_with("Something is already at "), "{error}");
    assert!(side.join("a.txt").exists());
    assert!(find(st.snap.as_ref().unwrap(), &side).is_some());
    cleanup(&dir, &side);
}

#[test]
fn browse_fills_the_move_dialog() {
    let (mut st, dir, side) = opened("wtmv-browse");
    drive(&mut st, wt(WorktreeMsg::MoveDialog(side.clone())));
    let parent = side.parent().unwrap().join("picked");
    drive(&mut st, Msg::Prefs(PrefsMsg::FolderPicked { purpose: FolderPurpose::WorktreeParent, path: Some(parent.clone()) }));
    assert_eq!(PathBuf::from(&form(&mut st).folder), parent.join("side"));
    cleanup(&dir, &side);
}

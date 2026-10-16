//! Headless tests for worktree tab groups: a worktree's new tab opens beside its group (straight
//! away when a snapshot lists it, after its load otherwise), restored tabs are pulled together
//! once their common dirs are read, a drop outside a group moves the whole group, and turning
//! the setting on regroups the open tabs.

use super::tests::{drive, finish_keeping, git, same_place, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, Tab};
use crate::worktree::WorktreeMsg;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::CommonDir;

/// A repository and a linked worktree of it (on branch `side`), next to it.
fn with_worktree(name: &str) -> (PathBuf, PathBuf) {
    let main = scratch_repo(name);
    let linked = linked_of(&main);
    git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    (main, linked)
}

fn linked_of(main: &Path) -> PathBuf {
    let linked = main.with_file_name(format!("{}-linked", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    linked
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn open(st: &mut AppState, dir: &Path) {
    drive(st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
}

/// Runs `msg` to completion and returns the sessions it saved.
fn saves_of(st: &mut AppState, msg: Msg) -> Vec<(Vec<PathBuf>, usize)> {
    let fx = update(st, msg, Instant::now());
    saved(&finish_keeping(st, fx))
}

fn saved(fx: &[Effect]) -> Vec<(Vec<PathBuf>, usize)> {
    fx.iter().filter_map(|e| if let Effect::SaveTabs { paths, active } = e { Some((paths.clone(), *active)) } else { None }).collect()
}

fn tab_is(st: &AppState, i: usize, dir: &Path) -> bool {
    st.tabs[i].path.as_deref().is_some_and(|p| same_place(p, dir))
}

fn order_is(paths: &[PathBuf], dirs: &[&Path]) -> bool {
    paths.len() == dirs.len() && paths.iter().zip(dirs).all(|(p, d)| same_place(p, d))
}

#[test]
fn a_worktree_opens_beside_its_repository() {
    for on in [true, false] {
        let (main, linked) = with_worktree(&format!("groups-open-{on}"));
        let other = scratch_repo(&format!("groups-open-other-{on}"));
        let mut st = state();
        st.settings.group_worktree_tabs = on;
        open(&mut st, &main);
        open(&mut st, &other);
        drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));

        let saves = saves_of(&mut st, Msg::Worktree(WorktreeMsg::Open(linked.clone())));
        let at = if on { 1 } else { 2 };
        assert_eq!((st.tabs.len(), st.active_tab), (3, at), "grouping {on}");
        assert!(tab_is(&st, at, &linked) && same_place(&st.snap.as_ref().unwrap().workdir, &linked));
        let (paths, active) = saves.last().expect("the session is saved");
        let want: [&Path; 3] = if on { [&main, &linked, &other] } else { [&main, &other, &linked] };
        assert!(order_is(paths, &want) && *active == at, "{paths:?}");
        cleanup(&[&linked, &main, &other]);
    }
}

#[test]
fn the_main_worktree_opens_beside_a_linked_one() {
    let (main, linked) = with_worktree("groups-main");
    let other = scratch_repo("groups-main-other");
    let mut st = state();
    open(&mut st, &linked);
    open(&mut st, &other);
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, Msg::Worktree(WorktreeMsg::Open(main.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (3, 1));
    assert!(tab_is(&st, 0, &linked) && tab_is(&st, 1, &main) && tab_is(&st, 2, &other));
    cleanup(&[&linked, &main, &other]);
}

#[test]
fn a_worktree_found_only_by_its_load_moves_beside_its_group() {
    let main = scratch_repo("groups-late");
    let other = scratch_repo("groups-late-other");
    let mut st = state();
    open(&mut st, &main);
    open(&mut st, &other);
    st.draft.summary = "other's draft".into();
    // Made after main's snapshot was read: no snapshot lists it.
    let linked = linked_of(&main);
    git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);

    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(linked.clone())), Instant::now());
    assert_eq!((st.tabs.len(), st.active_tab), (3, 2), "opens at the end first");
    finish_keeping(&mut st, fx);
    assert_eq!(st.active_tab, 1, "then moves beside main, still active");
    assert!(tab_is(&st, 0, &main) && tab_is(&st, 1, &linked) && tab_is(&st, 2, &other));
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, &linked));
    let parked = st.tabs[2].parked.as_ref().expect("other is parked");
    assert_eq!(parked.draft.summary, "other's draft", "parked data travels with its tab");
    cleanup(&[&linked, &main, &other]);
}

#[test]
fn a_failed_open_in_the_middle_returns_to_its_tab() {
    let (main, linked) = with_worktree("groups-fail");
    let other = scratch_repo("groups-fail-other");
    let mut st = state();
    open(&mut st, &main);
    open(&mut st, &other);
    // Main's snapshot still lists it, so its tab goes in at 1, before the active one.
    std::fs::remove_dir_all(&linked).unwrap();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(linked.clone())), Instant::now());
    assert_eq!((st.tabs.len(), st.active_tab, st.tab_return), (3, 1, Some(2)));
    finish_keeping(&mut st, fx);
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    assert!(tab_is(&st, 1, &other) && same_place(&st.snap.as_ref().unwrap().workdir, &other));
    cleanup(&[&main, &other]);
}

#[test]
fn restored_tabs_are_pulled_together_once_read() {
    let (main, linked) = with_worktree("groups-restore");
    let other = scratch_repo("groups-restore-other");
    for on in [true, false] {
        let mut st = state();
        st.settings.group_worktree_tabs = on;
        let paths = vec![main.clone(), other.clone(), linked.clone()];
        let fx = update(&mut st, Msg::Tabs(TabsMsg::RestoreTabs { paths: paths.clone(), active: 1 }), Instant::now());
        let read = fx.iter().find_map(|e| if let Effect::ReadTabGroups(p) = e { Some(p.clone()) } else { None });
        assert_eq!(read.as_ref(), Some(&paths), "every restored tab is read");
        let Some(Msg::Tabs(TabsMsg::GroupsRead(mut results))) = crate::runner::execute(Effect::ReadTabGroups(paths)) else {
            panic!("the read reports GroupsRead")
        };
        // A tab closed meanwhile.
        results.push((main.with_file_name("closed-meanwhile"), results[0].1.clone()));
        let fx = update(&mut st, Msg::Tabs(TabsMsg::GroupsRead(results)), Instant::now());
        assert!(st.tabs.iter().all(|t| t.group.is_some()), "groups recorded ({on})");
        assert_eq!(st.tabs.len(), 3);
        if on {
            assert!(tab_is(&st, 0, &main) && tab_is(&st, 1, &linked) && tab_is(&st, 2, &other));
            assert_eq!(st.active_tab, 2, "the active repository stays active");
            assert_eq!(saved(&fx).len(), 1);
        } else {
            assert!(tab_is(&st, 0, &main) && tab_is(&st, 1, &other) && tab_is(&st, 2, &linked));
            assert_eq!(st.active_tab, 1);
            assert!(saved(&fx).is_empty());
        }
    }

    // A group a load found is kept.
    let mut st = state();
    update(&mut st, Msg::Tabs(TabsMsg::RestoreTabs { paths: vec![main.clone(), other.clone()], active: 0 }), Instant::now());
    let known = group("loaded");
    st.tabs[0].group = Some(known.clone());
    let read = vec![(main.clone(), CommonDir::at(&main)), (other.clone(), CommonDir::at(&other))];
    update(&mut st, Msg::Tabs(TabsMsg::GroupsRead(read)), Instant::now());
    assert_eq!(st.tabs[0].group.as_ref(), Some(&known));
    cleanup(&[&linked, &main, &other]);
}

fn group(g: &str) -> CommonDir {
    CommonDir { dir: trellis_core::paths::temp_dir().join(g).join(".git"), folder: None, name: g.into() }
}

/// Unloaded tabs for made-up paths: "a1" and "a2" are of group "a"; one-letter names have none.
fn fake_tabs(names: &[&str], active: usize) -> AppState {
    let mut st = state();
    let base = trellis_core::paths::temp_dir().join("trellis-fake-tabs");
    st.tabs = names
        .iter()
        .map(|n| {
            let mut t = Tab::unloaded(base.join(n));
            t.group = (n.len() > 1).then(|| group(&n[..1]));
            t
        })
        .collect();
    st.active_tab = active;
    st
}

fn names(st: &AppState) -> Vec<&str> {
    st.tabs.iter().map(|t| t.name.as_str()).collect()
}

#[test]
fn drops_inside_and_outside_a_group() {
    let mut st = fake_tabs(&["a1", "a2", "x", "b1", "b2"], 0);
    st.tabs[3].parked = Some(Box::default());
    st.tabs[3].parked.as_mut().unwrap().draft.summary = "b1's draft".into();
    let fx = update(&mut st, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 1 }), Instant::now());
    assert_eq!((names(&st), st.active_tab), (vec!["a2", "a1", "x", "b1", "b2"], 1), "reordered inside the group");
    assert_eq!(saved(&fx).len(), 1);

    update(&mut st, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 3 }), Instant::now());
    assert_eq!((names(&st), st.active_tab), (vec!["x", "b1", "b2", "a2", "a1"], 4), "the whole group moves, the active tab with it");
    assert_eq!(st.tabs[1].parked.as_ref().unwrap().draft.summary, "b1's draft");

    update(&mut st, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 2 }), Instant::now());
    assert_eq!(names(&st), ["b1", "b2", "x", "a2", "a1"], "a lone tab never lands inside a group");
    update(&mut st, Msg::Tabs(TabsMsg::MoveTab { from: 2, to: 0 }), Instant::now());
    assert_eq!(names(&st), ["x", "b1", "b2", "a2", "a1"]);
    for (from, to) in [(1, 1), (9, 0), (0, 9)] {
        assert!(update(&mut st, Msg::Tabs(TabsMsg::MoveTab { from, to }), Instant::now()).is_empty(), "{from} -> {to}");
    }
}

#[test]
fn turning_grouping_on_regroups_once() {
    let mut st = fake_tabs(&["a1", "x", "a2", "b1", "y", "b2"], 2);
    st.settings.group_worktree_tabs = false;
    assert!(saved(&update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now())).is_empty());
    assert_eq!(names(&st), ["a1", "x", "a2", "b1", "y", "b2"], "off: left as it is");
    st.settings.group_worktree_tabs = true;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    assert_eq!(names(&st), ["a1", "a2", "x", "b1", "b2", "y"]);
    assert_eq!(st.active_tab, 1, "a2 stays active");
    assert_eq!(saved(&fx).len(), 1);
    assert!(saved(&update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now())).is_empty(), "nothing more to do");
}

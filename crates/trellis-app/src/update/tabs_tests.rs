//! Headless tests for tabs: each repository keeps its own state, results for other tabs are
//! dropped, and tabs close, reopen and restore as expected.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::tabs::MAX_CLOSED_TABS;
use crate::state::{AppState, Tab};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::DiffTarget;

fn active_is(st: &AppState, dir: &Path) -> bool {
    let tab = &st.tabs[st.active_tab];
    let same = super::tests::same_place;
    tab.path.as_deref().is_some_and(|p| same(p, dir)) && st.snap.as_ref().is_some_and(|s| same(&s.workdir, dir))
}

fn two_repos(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (a, b) = (scratch_repo(&format!("{name}-a")), scratch_repo(&format!("{name}-b")));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    (st, a, b)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn each_tab_keeps_its_own_repository_and_draft() {
    let a = scratch_repo("tabs-own-a");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert_eq!(st.tabs.len(), 1, "the empty start tab is reused");
    st.draft.summary = "work in a".into();
    // A's open diff, scrolled down a few rows.
    st.diff = Some((
        DiffTarget::Unstaged("a.txt".into()),
        trellis_core::repo::FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
            .into(),
    ));
    st.view.diff_top_row = 6;

    let b = scratch_repo("tabs-own-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    assert!(active_is(&st, &b));
    assert!(st.draft.summary.is_empty());
    assert!(st.diff.is_none() && st.view.diff_scroll_to_row.is_none());

    // Switching back shows A immediately (then refreshes it) with its draft intact.
    let fx = update(&mut st, Msg::SelectTab(0), Instant::now());
    assert!(active_is(&st, &a));
    assert_eq!(st.draft.summary, "work in a");
    assert_eq!(st.view.diff_scroll_to_row, Some(6), "A's diff reopens where it was scrolled");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })));

    // Opening an already open repository selects its tab.
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    // Ctrl+Tab wraps around.
    update(&mut st, Msg::CycleTab(1), Instant::now());
    assert_eq!(st.active_tab, 0);
    update(&mut st, Msg::CycleTab(-1), Instant::now());
    assert_eq!(st.active_tab, 1);
    cleanup(&[&a, &b]);
}

#[test]
fn results_for_other_tabs_are_dropped() {
    let (mut st, a, b) = two_repos("tabs-scope");
    let diff = Msg::Repo(RepoMsg::DiffLoaded {
        target: DiffTarget::Unstaged("a.txt".into()),
        reload: false,
        generation: 0,
        result: Err("from A".into()),
    });
    update(&mut st, Msg::Scoped { workdir: a.clone(), msg: Box::new(diff) }, Instant::now());
    assert!(!st.log.iter().any(|l| l.text == "from A"), "tab A's diff result must not reach tab B");

    // A git job started in A finishes after switching to B: B is not refreshed, A's draft is
    // cleared, and the result is still reported.
    drive(&mut st, Msg::SelectTab(0));
    drive(&mut st, Msg::StageFile("a.txt".into()));
    st.draft.summary = "commit me".into();
    update(&mut st, Msg::Commit, Instant::now());
    update(&mut st, Msg::SelectTab(1), Instant::now());
    let fx = update(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None }, Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Commit succeeded"));
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert!(parked.draft.summary.is_empty());
    cleanup(&[&a, &b]);
}

#[test]
fn close_and_reopen_tabs() {
    let (mut st, a, b) = two_repos("tabs-close");
    drive(&mut st, Msg::CloseTab(usize::MAX));
    assert_eq!(st.tabs.len(), 1);
    assert!(active_is(&st, &a));

    drive(&mut st, Msg::ReopenClosedTab);
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    assert!(active_is(&st, &b));

    // Closing an inactive tab keeps the active one.
    drive(&mut st, Msg::CloseTab(0));
    assert_eq!((st.tabs.len(), st.active_tab), (1, 0));
    assert!(active_is(&st, &b));

    // Closing the last tab leaves an empty one (the welcome screen).
    drive(&mut st, Msg::CloseTab(0));
    assert_eq!(st.tabs.len(), 1);
    assert!(st.snap.is_none() && st.tabs[0].path.is_none());
    cleanup(&[&a, &b]);
}

#[test]
fn a_failed_open_returns_to_the_previous_tab() {
    let a = scratch_repo("tabs-fail");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.join("no-such-folder"))));
    assert_eq!(st.tabs.len(), 1);
    assert!(active_is(&st, &a));
    assert!(st.log.last().is_some_and(|l| !l.ok));
    cleanup(&[&a]);
}

#[test]
fn restored_tabs_load_when_first_selected() {
    let (a, b) = (scratch_repo("tabs-restore-a"), scratch_repo("tabs-restore-b"));
    let mut st = state();
    let fx = update(&mut st, Msg::RestoreTabs { paths: vec![a.clone(), b.clone()], active: 1 }, Instant::now());
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "the empty start tab is replaced");
    let loads: Vec<&PathBuf> = fx.iter().filter_map(|e| if let Effect::LoadRepo { path, .. } = e { Some(path) } else { None }).collect();
    assert_eq!(loads, [&b], "only the active tab loads at startup");
    assert_eq!(st.tabs[0].name, a.file_name().unwrap().to_string_lossy());

    let fx = update(&mut st, Msg::SelectTab(0), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, kind: LoadKind::Open, .. } if path == &a)));
    match fx.iter().find_map(|e| if let Effect::SaveTabs { paths, active } = e { Some((paths, *active)) } else { None }) {
        Some((paths, active)) => assert_eq!((paths.len(), active), (2, 0)),
        None => panic!("switching tabs saves the session"),
    }
    cleanup(&[&a, &b]);
}

#[test]
fn a_repository_opened_through_a_link_is_the_same_tab() {
    // CI's temp folder has a short (8.3) name; a junction or symlink exercises the same thing:
    // the path the user opened differs from the one libgit2 reports.
    let a = scratch_repo("tabs-link");
    let link = a.with_extension("link");
    let _ = std::fs::remove_dir_all(&link);
    #[cfg(windows)]
    let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&a).output().is_ok_and(|o| o.status.success());
    #[cfg(not(windows))]
    let made = std::os::unix::fs::symlink(&a, &link).is_ok();
    assert!(made, "could not link {}", link.display());

    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(link.clone())));
    assert!(active_is(&st, &a) && active_is(&st, &link));
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert_eq!(st.tabs.len(), 1, "opening the real folder switches to the same tab");

    // The other way round, from a new tab: the link's name matches nothing until it loads,
    // then its tab gives way to the one already showing the folder.
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::NewTab);
    drive(&mut st, Msg::Repo(RepoMsg::Open(link.clone())));
    assert_eq!(st.tabs.len(), 1, "the link's tab closes in favour of the real folder's");
    assert!(active_is(&st, &a) && st.tabs[0].real.is_some());
    #[cfg(windows)]
    let _ = std::fs::remove_dir(&link);
    #[cfg(not(windows))]
    let _ = std::fs::remove_file(&link);
    cleanup(&[&a]);
}

#[test]
fn conflict_view_survives_tab_switch() {
    use trellis_core::repo::{Choice, Side};
    let a = super::conflict_tests::conflicted_repo("tabs-conflict-a");
    let b = scratch_repo("tabs-conflict-b");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs }), Instant::now());
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.conflict.is_none(), "the other tab has no conflict open");
    drive(&mut st, Msg::SelectTab(0));
    assert_eq!(st.conflict.as_ref().map(|c| c.choices.clone()), Some(vec![Choice::Theirs]), "picks survive the switch");

    // A load still in flight when the tab is parked is requested again on return.
    update(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())), Instant::now());
    drive(&mut st, Msg::SelectTab(1));
    let fx = update(&mut st, Msg::SelectTab(0), Instant::now());
    let generation = st.conflict_gen;
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadConflict { path, generation: g, .. } if path == "a.txt" && *g == generation)));
    cleanup(&[&a, &b]);
}

/// Unloaded tabs for made-up paths (nothing is read from disk), with tab `active` selected.
fn fake_tabs(names: &[&str], active: usize) -> AppState {
    let mut st = state();
    st.tabs = names.iter().map(|n| Tab::unloaded(fake(n))).collect();
    st.active_tab = active;
    st
}

fn fake(name: &str) -> PathBuf {
    PathBuf::from(format!("C:/r/{name}"))
}

fn tab_names(st: &AppState) -> Vec<String> {
    st.tabs.iter().map(|t| t.name.clone()).collect()
}

fn saved(fx: &[Effect]) -> Vec<(Vec<PathBuf>, usize)> {
    fx.iter().filter_map(|e| if let Effect::SaveTabs { paths, active } = e { Some((paths.clone(), *active)) } else { None }).collect()
}

#[test]
fn move_tab_keeps_the_active_repository() {
    let mut st = fake_tabs(&["a", "b", "c"], 1);
    st.draft.summary = "wip".into();
    let load_gen = st.load_gen;
    let fx = update(&mut st, Msg::MoveTab { from: 1, to: 2 }, Instant::now());
    assert_eq!(tab_names(&st), ["a", "c", "b"]);
    assert_eq!(st.active_tab, 2);
    assert_eq!(st.draft.summary, "wip");
    assert_eq!(st.load_gen, load_gen);
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. } | Effect::Ui(UiEffect::SetTitle(_)))));
    assert_eq!(fx.len(), 1);
    assert_eq!(saved(&fx), [(vec![fake("a"), fake("c"), fake("b")], 2)]);
}

#[test]
fn moving_another_tab_across_the_active_one_shifts_active_tab() {
    let mut st = fake_tabs(&["a", "b", "c"], 1);
    update(&mut st, Msg::MoveTab { from: 0, to: 2 }, Instant::now());
    assert_eq!((tab_names(&st), st.active_tab), (vec!["b".to_owned(), "c".into(), "a".into()], 0));
    update(&mut st, Msg::MoveTab { from: 2, to: 0 }, Instant::now());
    assert_eq!((tab_names(&st), st.active_tab), (vec!["a".to_owned(), "b".into(), "c".into()], 1));
    for (from, to) in [(1, 1), (5, 0), (0, 5)] {
        let fx = update(&mut st, Msg::MoveTab { from, to }, Instant::now());
        assert!(fx.is_empty(), "MoveTab {{ {from}, {to} }} is a no-op");
        assert_eq!((tab_names(&st), st.active_tab), (vec!["a".to_owned(), "b".into(), "c".into()], 1));
    }
}

#[test]
fn move_tab_remaps_tab_return() {
    let mut st = fake_tabs(&["a", "b", "c"], 1);
    st.tab_return = Some(0);
    update(&mut st, Msg::MoveTab { from: 0, to: 2 }, Instant::now());
    assert_eq!(st.tab_return, Some(2));
}

#[test]
fn close_other_tabs_keeping_an_inactive_tab() {
    let (mut st, a, b) = two_repos("tabs-close-others");
    assert!(active_is(&st, &b));
    let fx = update(&mut st, Msg::CloseOtherTabs(0), Instant::now());
    assert_eq!(st.tabs.len(), 1);
    assert!(active_is(&st, &a), "A's parked snapshot is restored");
    assert_eq!(st.closed_tabs.len(), 1);
    assert!(super::tests::same_place(&st.closed_tabs[0], &b));
    let saves = saved(&fx);
    assert_eq!(saves.len(), 1);
    assert!(saves[0].0.len() == 1 && super::tests::same_place(&saves[0].0[0], &a));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })));
    cleanup(&[&a, &b]);
}

#[test]
fn close_other_tabs_keeping_the_active_tab() {
    let mut st = fake_tabs(&["a", "b", "c", "d"], 2);
    let fx = update(&mut st, Msg::CloseOtherTabs(2), Instant::now());
    assert_eq!((tab_names(&st), st.active_tab), (vec!["c".to_owned()], 0));
    assert_eq!(st.closed_tabs, [fake("d"), fake("b"), fake("a")], "a is reopened first");
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })));
    assert_eq!(saved(&fx), [(vec![fake("c")], 0)]);
    // With one tab left there is nothing to close.
    assert!(update(&mut st, Msg::CloseOtherTabs(0), Instant::now()).is_empty());
    assert!(update(&mut st, Msg::CloseOtherTabs(4), Instant::now()).is_empty());
}

#[test]
fn close_tabs_to_the_right() {
    let mut st = fake_tabs(&["a", "b", "c", "d"], 3);
    let fx = update(&mut st, Msg::CloseTabsToRight(1), Instant::now());
    assert_eq!((tab_names(&st), st.active_tab), (vec!["a".to_owned(), "b".into()], 1));
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, kind: LoadKind::Open, .. } if path == &fake("b"))),
        "the survivor was never loaded"
    );
    assert!(update(&mut st, Msg::CloseTabsToRight(1), Instant::now()).is_empty());
    assert!(update(&mut st, Msg::CloseTabsToRight(9), Instant::now()).is_empty());
    assert_eq!(st.tabs.len(), 2);
}

#[test]
fn bulk_closed_tabs_reopen_in_their_original_order() {
    let mut st = fake_tabs(&["a", "b", "c", "d"], 0);
    let fx = update(&mut st, Msg::CloseTabsToRight(1), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "the active tab stays");
    update(&mut st, Msg::ReopenClosedTab, Instant::now());
    update(&mut st, Msg::ReopenClosedTab, Instant::now());
    let paths: Vec<PathBuf> = st.tabs.iter().filter_map(|t| t.path.clone()).collect();
    assert_eq!(paths, [fake("a"), fake("b"), fake("c"), fake("d")]);
}

#[test]
fn bulk_close_skips_empty_tabs_and_caps_the_stack() {
    let mut st = fake_tabs(&["x", "y"], 0);
    st.tabs.insert(1, Tab::empty());
    st.tabs.push(Tab::empty());
    update(&mut st, Msg::CloseOtherTabs(0), Instant::now());
    assert_eq!(st.closed_tabs, [fake("y")], "empty tabs are not remembered");

    let names: Vec<String> = (0..13).map(|i| format!("t{i}")).collect();
    let mut st = fake_tabs(&names.iter().map(String::as_str).collect::<Vec<_>>(), 0);
    update(&mut st, Msg::CloseTabsToRight(0), Instant::now());
    assert_eq!(st.closed_tabs.len(), MAX_CLOSED_TABS);
    assert_eq!(st.closed_tabs.last(), Some(&fake("t1")), "the leftmost closed tab reopens first");
    assert_eq!(st.closed_tabs[0], fake("t10"), "the rightmost ones drop off");
}

//! Headless tests for other worktrees' WIP rows in the graph: a changed worktree gets a row above
//! its HEAD commit, a status-only reload keeps it, the setting drops and restores it through a
//! full load, opening the row's worktree opens or switches to its tab; and the rows' text rules.

use super::tests::{drive, finish, git, same_place, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg, RepoMsg, TabsMsg, WatchMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::worktrees::{counts_text, wip_label, wip_tooltip};
use crate::worktree::WorktreeMsg;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{ChangeCounts, ChangesRead, WorktreeInfo};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// scratch_repo (a.txt modified) with a linked worktree on `side` beside it that has a.txt
/// modified and b.txt added, opened in `st` on the main worktree.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    std::fs::write(linked.join("a.txt"), "changed there\n").unwrap();
    std::fs::write(linked.join("b.txt"), "new there\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, linked)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// (worktree, before) of each worktree WIP row.
fn wip_rows(st: &AppState) -> Vec<(usize, usize)> {
    st.snap.as_ref().unwrap().graph.worktree_wips.iter().map(|w| (w.worktree, w.before)).collect()
}

/// Runs `msg` and the loads it leads to, returning each load effect (as debug text) on the way.
fn loads(st: &mut AppState, msg: Msg) -> Vec<String> {
    let mut seen = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for e in update(st, m, Instant::now()) {
            if matches!(e, Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::LoadDiff { .. }) {
                seen.push(format!("{e:?}"));
                queue.extend(runner::execute(e));
            }
        }
    }
    seen
}

#[test]
fn a_changed_worktree_gets_a_row_above_its_head() {
    let (st, dir, linked) = opened("wtwip-row");
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.worktrees.len(), 2);
    assert_eq!(snap.worktrees[1].changes, ChangeCounts { modified: 1, added: 1, deleted: 0 });
    let head = snap.head_oid.unwrap();
    assert_eq!(wip_rows(&st), [(1, snap.row_of[&head])]);
    let rows = GraphRows::new(snap, &st.session.search, false);
    assert_eq!(rows.len(), snap.commits.len() + 2);
    assert_eq!((rows.at(0), rows.at(1), rows.at(2)), (Some(DisplayRow::Wip), Some(DisplayRow::Worktree(0)), Some(DisplayRow::Commit(0))));
    cleanup(&[&linked, &dir]);
}

#[test]
fn a_status_read_keeps_the_worktree_rows() {
    let (mut st, dir, linked) = opened("wtwip-status");
    assert!(st.snap.as_ref().unwrap().graph.wip.is_some());
    git(&dir, &["checkout", "--", "a.txt"]);
    let workdir = st.workdir().unwrap();
    let changed = Msg::Watch(WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() });
    let seen = loads(&mut st, Msg::Scoped { workdir, msg: Box::new(changed) });
    assert!(seen.iter().any(|e| e.starts_with("LoadStatus")), "{seen:?}");
    assert!(!seen.iter().any(|e| e.starts_with("LoadRepo")), "{seen:?}");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.wip.is_none(), "the own WIP row is gone");
    assert_eq!(wip_rows(&st), [(1, 0)], "the other worktree's row stays");
    cleanup(&[&linked, &dir]);
}

#[test]
fn turning_the_setting_off_drops_the_rows() {
    let (mut st, dir, linked) = opened("wtwip-setting");
    assert_eq!(wip_rows(&st).len(), 1);
    st.settings.worktree_wips = false;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), Instant::now());
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, known: None, .. } if !opts.worktree_wips)),
        "a full load without the rows: {fx:?}"
    );
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.worktree_wips.is_empty());
    assert!(snap.worktrees.iter().all(|w| w.changes.is_empty()));

    st.settings.worktree_wips = true;
    drive(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert_eq!(wip_rows(&st).len(), 1, "back on: the row is back");
    cleanup(&[&linked, &dir]);
}

#[test]
fn the_row_opens_or_switches_to_the_worktree_tab() {
    let (mut st, dir, linked) = opened("wtwip-open");
    let path = st.snap.as_ref().unwrap().worktrees[1].path.clone();
    drive(&mut st, Msg::Worktree(WorktreeMsg::Open(path.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, &linked));

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.active_tab, 0);
    drive(&mut st, Msg::Worktree(WorktreeMsg::Open(path)));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "switched to the open tab");
    cleanup(&[&linked, &dir]);
}

fn info(branch: Option<&str>, locked: Option<&str>) -> WorktreeInfo {
    WorktreeInfo {
        name: Some("feature-login".into()),
        path: PathBuf::from("/work/app-worktrees/feature-login"),
        branch: branch.map(str::to_owned),
        head: Some(Oid::from_str("1234567890abcdef1234567890abcdef12345678").unwrap()),
        current: false,
        locked: locked.map(str::to_owned),
        missing: false,
        prunable: false,
        folder: None,
        changes: ChangeCounts { modified: 2, added: 0, deleted: 3 },
        read: ChangesRead::Counted,
    }
}

#[test]
fn worktree_wip_text() {
    assert_eq!(counts_text(ChangeCounts { modified: 2, added: 0, deleted: 3 }), "2 modified, 3 deleted");
    assert_eq!(counts_text(ChangeCounts { modified: 2, added: 1, deleted: 3 }), "2 modified, 1 added, 3 deleted");
    assert_eq!(counts_text(ChangeCounts { modified: 0, added: 4, deleted: 0 }), "4 added");
    let wt = info(Some("feature/login"), Some("on usb"));
    assert_eq!(wip_label(Lang::En, &wt), "Uncommitted changes · feature-login");
    assert_eq!(wip_label(Lang::De, &wt), "Nicht committete Änderungen · feature-login");
    let tip = wip_tooltip(&wt);
    let lines: Vec<&str> = tip.lines().collect();
    assert_eq!(lines[0], "Uncommitted changes in feature-login");
    assert_eq!(lines[1], glowfly_core::native_path::shown(&wt.path));
    assert_eq!(&lines[2..], ["Branch: feature/login", "2 modified, 3 deleted", "Locked: on usb", "Click to open it in a tab."]);
    let detached = wip_tooltip(&info(None, Some("")));
    assert!(detached.contains("Detached HEAD at 1234567") && detached.contains("\nLocked\n"), "{detached}");
    assert!(!wip_tooltip(&info(None, None)).contains("Locked"));
}

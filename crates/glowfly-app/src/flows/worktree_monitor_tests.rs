//! Removing or moving a worktree on Windows stops its file system monitor daemon first (it holds
//! the folder open there): the request says so whenever this OS and git have the daemon. The
//! runner's side, the stop running before git, is in runner/worktrees_tests.rs.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, WorktreeEffect};
use crate::msg::{Msg, RepoMsg};
use crate::state::git_env::GitState;
use crate::state::{AppState, Modal};
use crate::worktree::WorktreeMsg;
use glowfly_core::git_version::GitInfo;
use glowfly_core::opens::Os;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn git_of(version: (u32, u32, u32), daemon: Option<bool>) -> GitState {
    let info = GitInfo { path: Some("git".into()), version: Some(version), chosen: false };
    GitState { info: Some(info), monitor_daemon: daemon, ..GitState::default() }
}

#[test]
fn only_windows_with_the_daemon_stops_it() {
    let new = (2, 45, 0);
    assert!(git_of(new, Some(true)).stops_monitor_for(Os::Windows));
    assert!(git_of(new, None).stops_monitor_for(Os::Windows), "not probed yet: a stop costs one git run");
    assert!(!git_of(new, Some(false)).stops_monitor_for(Os::Windows), "built without it");
    assert!(!git_of((2, 35, 1), Some(true)).stops_monitor_for(Os::Windows), "too old to have it");
    for os in [Os::MacOs, Os::Linux] {
        assert!(!git_of(new, Some(true)).stops_monitor_for(os), "{os:?} lets the folder go");
    }
    assert_eq!(git_of(new, Some(true)).stops_monitor(), cfg!(windows));
}

/// A repository opened in `st` with git 2.45 that has the daemon, and a linked worktree `side`.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let side = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&side);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &side.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.git = git_of((2, 45, 0), Some(true));
    (st, dir, side)
}

fn cleanup(dir: &Path, side: &Path) {
    let _ = std::fs::remove_dir_all(side);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_remove_asks_for_the_stop() {
    for daemon in [Some(true), Some(false)] {
        let (mut st, dir, side) = opened("wt-monitor-remove");
        st.git.monitor_daemon = daemon;
        drive(&mut st, Msg::Worktree(WorktreeMsg::RemoveDialog(side.clone())));
        assert!(matches!(st.modal, Some(Modal::RemoveWorktree(_))), "{:?}", st.log.last().map(|l| &l.text));
        let fx = update(&mut st, Msg::Worktree(WorktreeMsg::Remove), Instant::now());
        let Some(Effect::Worktree(WorktreeEffect::Remove(r))) = fx.iter().find(|e| matches!(e, Effect::Worktree(_))) else {
            panic!("{fx:?}")
        };
        assert_eq!(r.stop_monitor, cfg!(windows) && daemon == Some(true), "{daemon:?}");
        cleanup(&dir, &side);
    }
}

#[test]
fn a_move_asks_for_the_stop() {
    for daemon in [Some(true), Some(false)] {
        let (mut st, dir, side) = opened("wt-monitor-move");
        st.git.monitor_daemon = daemon;
        drive(&mut st, Msg::Worktree(WorktreeMsg::MoveDialog(side.clone())));
        let Some(Modal::MoveWorktree(form)) = st.modal.as_mut() else { panic!("{:?}", st.log.last().map(|l| &l.text)) };
        form.folder = side.with_file_name("wt-monitor-moved").display().to_string();
        let fx = update(&mut st, Msg::Worktree(WorktreeMsg::Move), Instant::now());
        let [Effect::Worktree(WorktreeEffect::Move(r))] = &fx[..] else { panic!("{fx:?}") };
        assert_eq!(r.stop_monitor, cfg!(windows) && daemon == Some(true), "{daemon:?}");
        cleanup(&dir, &side);
    }
}
